use std::{
    env,
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    str::FromStr,
};
use serde_json::json;
use tokio::{net::{TcpListener, TcpStream}, time::timeout};
use tokio::time::{self, Duration};
use tokio::sync::mpsc::channel;

use futures_util::{FutureExt, SinkExt, StreamExt, future, pin_mut, stream::{
        TryStreamExt,
    }};
use jsonrpc_core::{MetaIoHandler, Metadata, Params};
use tokio_tungstenite::tungstenite::Message;
use ethers::prelude::*;

use crate::{
    config::Config,
    game,
    authenticate,
    crypto::entry_fee_paid_event_listener,
    game_pool::game_pool_reward_added_listener,
};

use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug)]
struct Listener {
    listener: TcpListener,
    handler: Arc<MetaIoHandler<Meta>>,
    peer_map: crate::PeerMap,
    eth_addr_peer_map: crate::EthAddrPeerMap,
    game: crate::Game,
}

async fn handle_connection(
    game: crate::Game,
    handler: Arc<MetaIoHandler<Meta>>,
    peer_map: crate::PeerMap,
    eth_addr_peer_map: crate::EthAddrPeerMap,
    stream: TcpStream,
    addr: SocketAddr
) {
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("failed to establish websocket connection");

    let mut eth_address = String::new();
    let (mut outgoing, mut incoming) = ws_stream.split();
    let (tx, rx) = channel(128);
    let rx = ReceiverStream::new(rx);

    let mut authenticated = false;
    let res = timeout(Duration::from_secs(3), incoming.next()).await;
    match res {
        Ok(msg) => {
            if let Some(msg) = msg {
                if let Ok(msg) = msg.unwrap().into_text() {
                    if let Ok(request) = serde_json::from_str::<AuthRequest>(&msg) {
                        let response;
                        if eth_addr_peer_map.lock().unwrap().contains_key(&request.params.address) {
                            // check if address is already logged in
                            response = json!({
                                "id": request.id,
                                "jsonrpc": "2.0",
                                "error": jsonrpc_core::Error {
                                    code: jsonrpc_core::ErrorCode::ServerError(1000),
                                    message: String::from("This address is already connected from a different client"),
                                    data: None,
                                },
                            });
                        } else {
                            // check if signature is valid
                            authenticated = authenticate::authenticate(&request.params.address, &request.params.signature);
                            if authenticated {
                                eth_address = request.params.address;
                                response = json!({
                                    "id": request.id,
                                    "jsonrpc": "2.0",
                                    "result": serde_json::Value::Null,
                                });
                            } else {
                                response = json!({
                                    "id": request.id,
                                    "jsonrpc": "2.0",
                                    "error": jsonrpc_core::Error {
                                        code: jsonrpc_core::ErrorCode::ServerError(1000),
                                        message: String::from("Address cannot be recovered from signature"),
                                        data: None,
                                    },
                                });
                            }
                        }
                        outgoing.send(Message::Text(response.to_string())).await;
                    }
                }
            }
        }
        Err(_) => {
            println!("Connection authentication timed out. Addr [{}]", addr);
            return;
        }
    }

    if authenticated {
        peer_map.lock().unwrap().insert(addr, tx.clone());
        eth_addr_peer_map.lock().unwrap().insert(eth_address.clone(), tx.clone());

        let incoming_future = incoming.try_for_each(|msg| async {
            let msg = msg.into_text().unwrap(); // TODO: handle when message is not text
            // println!("Received message: {}", msg);
            let response = handler.handle_request(&msg, Meta(Some(addr), eth_address.clone())).await;
            if let Some(result) = response {
                // println!("Sending response {}", result);
                tx.send(Message::text(result)).await;
            }
            Ok(())
        });

        let outgoing_future = rx.map(Ok).forward(outgoing);
        pin_mut!(incoming_future, outgoing_future);
        future::select(outgoing_future, incoming_future).await;

        println!("Lost connection with client. Socket Address [{}]", addr);
        game.lock().unwrap().player_lost_connection(addr);
        peer_map.lock().unwrap().remove(&addr);
        eth_addr_peer_map.lock().unwrap().remove(&eth_address);
    } else {
        println!("Authentication failed for client. Socket Address [{}]", addr);
    }

}


pub async fn run(config: Config, listener: TcpListener) -> crate::Result<()> {
    let ws = loop {
        if let Ok(ws_) = Ws::connect(&config.provider_ws_url).await {
            println!("Connected to provider");
            break ws_;
        } else {
            println!("Failed to connect provider. Will attemp again in 3 seconds");
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    };
    let provider = Provider::new(ws).interval(Duration::from_millis(2000));

    let wallet = Wallet::from_str(&config.secret_key).expect("SECRET KEY is not valid").with_chain_id(config.chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);


    let peer_map = Arc::new(Mutex::new(HashMap::new()));
    let eth_addr_peer_map = Arc::new(Mutex::new(HashMap::new()));
    let game = Arc::new(Mutex::new(game::Game::new(
        peer_map.clone(),
        eth_addr_peer_map.clone(),
        config.no_entry_fee,
        config.multiplier as u32,
    )));

  if !config.no_entry_fee {
    tokio::spawn(
        entry_fee_paid_event_listener(
            config.fee_manager_address.clone(),
            config.game_pool_address.clone(),
            client.clone(),
            game.clone(),
        )
    );
  }

    tokio::spawn(
        game_pool_reward_added_listener(
            config.provider_http_url.clone(),
            game.clone(),
            config.game_pool_address.clone(),
        )
    );

    let win_tx = crate::game_pool::winner_listener(
        &config.game_pool_address,
        client.clone(),
    );

    game::start_tasks(
        game.clone(),
        config.multiplier,
        eth_addr_peer_map.clone(),
        client.clone(),
        win_tx,
    );

    let mut server = Listener {
        listener,
        handler: Arc::new(create_handler(game.clone())),
        peer_map,
        eth_addr_peer_map,
        game,
    };

    server.run().await;

    Ok(())
}


impl Listener {
    async fn run(&mut self) -> crate::Result<()> {

        loop {
            let (stream, addr) = self.accept().await?;
            tokio::spawn(
                handle_connection(
                    self.game.clone(),
                    self.handler.clone(),
                    self.peer_map.clone(),
                    self.eth_addr_peer_map.clone(),
                    stream,
                    addr,
                )
            );
        }
    }

    async fn accept(&mut self) -> crate::Result<(TcpStream, SocketAddr)> {
        let mut backoff = 1;
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => return Ok((stream, addr)),
                Err(err) => {
                    if backoff > 64 {
                        return Err(err.into());
                    }
                }
            }

            time::sleep(Duration::from_secs(backoff)).await;
            backoff *= 2;
        }
    }
}

use serde::{Deserialize};


#[derive(Deserialize)]
struct SetTargetParams {
    x: f64,
    y: f64,
}


#[derive(Debug, Clone, Default)]
struct Meta(Option<SocketAddr>, String);
impl Metadata for Meta {}


fn create_handler(game: crate::Game) -> MetaIoHandler<Meta> {
    let mut io = MetaIoHandler::default();

    let local_game = game.clone();
    io.add_method_with_meta("enter_game", move |_params: Params, meta: Meta| {
        let mut local_game = local_game.lock().unwrap();
        let res = local_game.enter_game(meta.0.unwrap(), meta.1);
        match res {
            Ok(_) => future::ok(jsonrpc_core::Value::Null),
            Err(err) => future::err(jsonrpc_core::Error {
                code: jsonrpc_core::ErrorCode::ServerError(1000),
                message: err.description(),
                data: None,
            })
        }
    });

    let local_game = game.clone();
    io.add_method_with_meta("get_available_tickets", move |_params: Params, meta: Meta| {
        let local_game = local_game.lock().unwrap();
        let res = local_game.get_available_tickets(&meta.1);
        future::ok(json!(res))
    });

    let local_game = game.clone();
    io.add_method_with_meta("get_server_info", move |_params: Params, meta: Meta| {
        let local_game = local_game.lock().unwrap();
        let res = local_game.get_server_info();
        future::ok(json!(res))
    });

    let local_game = game.clone();
    io.add_notification_with_meta("target", move |params: Params, meta: Meta| {
        if let Ok(parsed) = params.parse::<SetTargetParams>() {
            let mut local_game = local_game.lock().unwrap();
            local_game.set_target(meta.0.unwrap(), parsed.x, parsed.y);
        }

    });

    let local_game = game.clone();
    io.add_notification_with_meta("split", move |_params: Params, meta: Meta| {
        let mut local_game = local_game.lock().unwrap();
        local_game.split(meta.0.unwrap());
    });

    io
}

#[derive(Debug, Deserialize)]
struct AuthRequestParams {
    signature: String,
    address: String,
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    id: i32,
    jsonrpc: String,
    method: String,
    params: AuthRequestParams
}







