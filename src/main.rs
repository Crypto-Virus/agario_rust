
// #![feature(hash_drain_filter)]
// #![feature(drain_filter)]

// mod game;
// mod utils;




// use std::{
//     collections::HashMap,
//     env,
//     io::Error as IoError,
//     net::SocketAddr,
//     sync::{Arc, Mutex}
// };
// use futures_channel::mpsc::{unbounded, UnboundedSender};
// use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

// use tokio::net::{TcpListener, TcpStream};
// use tungstenite::protocol::Message;

// use jsonrpc_core::IoHandler;


// type Tx = UnboundedSender<Message>;
// type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
// type Handler = Arc<Mutex<IoHandler>>;


// async fn handle_connection(handler: Handler, peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
//     println!("Incoming TCP connection from: {}", addr);

//     let ws_stream = tokio_tungstenite::accept_async(raw_stream)
//         .await
//         .expect("error during websocket handshake occured");
//     println!("Websocket connection established: {}", addr);

//     // let (tx, rx) = unbounded();
//     // peer_map.lock().unwrap().insert(addr, tx);

//     let (outgoing, incoming) = ws_stream.split();

//     let broadcast_incoming = incoming.try_for_each(|msg| {
//         println!("Received a message from {}: {}", addr, msg.to_text().unwrap());
//         let peers = peer_map.lock().unwrap();
//         let io = handler.lock().unwrap();
//         let a = io.handle_request_sync(msg.to_text().unwrap());

//         let broadcast_recipients =
//             peers.iter().filter(|(peer_addr, _)| peer_addr != &&addr).map(|(_, ws_sink)| ws_sink);

//         for recp in broadcast_recipients {
//             recp.unbounded_send(msg.clone()).unwrap();
//         }

//         future::ok(())
//     });

//     // let receive_from_others = rx.map(Ok).forward(outgoing);

//     // pin_mut!(broadcast_incoming, receive_from_others);
//     pin_mut!(broadcast_incoming);
//     // future::select(broadcast_incoming, receive_from_others).await;
//     broadcast_incoming.await;


//     println!("{} disconnectd", &addr);
//     peer_map.lock().unwrap().remove(&addr);
// }

// use jsonrpc_core::*;



use tokio::net::TcpListener;

use agario_rust::server;

#[tokio::main]
async fn main() -> agario_rust::Result<()> {

    let port = 8080;
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await?;
    server::run(listener).await;
    Ok(())
}



