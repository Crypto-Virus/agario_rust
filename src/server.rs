use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Duration};
use futures_channel::mpsc::{unbounded};
use futures_util::{
    future,
    pin_mut,
    stream::{
        TryStreamExt,
    },
    StreamExt,
};
use jsonrpc_core::{MetaIoHandler, Metadata, Params};

use crate::game;



#[derive(Debug)]
struct Listener {
    listener: TcpListener,
    handler: Arc<MetaIoHandler<Meta>>,
    peer_map: crate::PeerMap,
    game: crate::Game,
}



async fn handle_connection(game: crate::Game, handler: Arc<MetaIoHandler<Meta>>, peer_map: crate::PeerMap, stream: TcpStream, addr: SocketAddr) {
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("failed to establish websocket connection");


    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let incoming_future = incoming.try_for_each(|msg| {
        println!("Recieved message. [{}]", msg.to_text().unwrap());
        handler.handle_request_sync(msg.to_text().unwrap(), Meta(Some(addr)));
        future::ok(())
    });

    let outgoing_future = rx.map(Ok).forward(outgoing);
    pin_mut!(incoming_future, outgoing_future);
    future::select(outgoing_future, incoming_future).await;

    println!("Lost connection with addr. Addr [{}]", addr);
    game.lock().unwrap().player_lost_connection(addr);
    peer_map.lock().unwrap().remove(&addr);

}


pub async fn run(listener: TcpListener) -> crate::Result<()> {
    let peer_map = Arc::new(Mutex::new(HashMap::new()));
    let game = Arc::new(Mutex::new(game::Game::new(peer_map.clone())));

    game::start_tasks(game.clone());

    let mut server = Listener {
        listener,
        handler: Arc::new(create_handler(game.clone())),
        peer_map,
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
struct EnterGameParams {
    player_name: String,
}

#[derive(Deserialize)]
struct SetTargetParams {
    dist: f64,
    rad: f64,
}


#[derive(Debug, Clone, Default)]
struct Meta(Option<SocketAddr>);
impl Metadata for Meta {}

fn create_handler(game: crate::Game) -> MetaIoHandler<Meta> {
    let mut io = MetaIoHandler::default();

    let local_game = game.clone();
    io.add_notification_with_meta("enter_game", move |params: Params, meta: Meta| {
        if let Ok(parsed) = params.parse::<EnterGameParams>() {
            let mut local_game = local_game.lock().unwrap();
            local_game.add_player(meta.0.unwrap(), parsed.player_name);
        }
    });

    let local_game = game.clone();
    io.add_notification_with_meta("target", move |params: Params, meta: Meta| {
        if let Ok(parsed) = params.parse::<SetTargetParams>() {
            let mut local_game = local_game.lock().unwrap();
            local_game.set_target(meta.0.unwrap(), parsed.dist, parsed.rad);
        }

    });

    let local_game = game.clone();
    io.add_notification_with_meta("split", move |params: Params, meta: Meta| {
        let mut local_game = local_game.lock().unwrap();
        local_game.split(meta.0.unwrap());
    });

    io
}