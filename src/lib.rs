
#![feature(hash_drain_filter)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    net::SocketAddr,
};

use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

pub mod config;
pub mod game;
pub mod server;
pub mod utils;
pub mod grid;
pub mod crypto;
pub mod game_pool;
pub mod authenticate;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, Sender<Message>>>>;
pub type EthAddrPeerMap = Arc<Mutex<HashMap<String, Sender<Message>>>>;
pub type Game = Arc<Mutex<game::Game>>;
