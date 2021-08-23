
use std::{
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use ethers::prelude::*;
use tokio::time::sleep;

use crate::Game;

pub async fn play_events_listener(game: Game) -> anyhow::Result<()> {
    let ws = loop {
        if let Ok(ws_) = Ws::connect("ws://localhost:8545").await {
            println!("Connected to provider");
            break ws_;
        } else {
            println!("Failed to connect provider. Will attemp again in 3 seconds");
            sleep(Duration::from_secs(3)).await;
        }
    };
    let provider = Provider::new(ws).interval(Duration::from_millis(2000));

    let contract_addr = H160::from_str("0x5fbdb2315678afecb367f032d93f642f64180aa3").unwrap();
    let secret_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let wallet = Wallet::from_str(secret_key).unwrap().with_chain_id(31337u64);
    let client = SignerMiddleware::new(provider, wallet);
    let client = Arc::new(client);

    abigen!(
        SimpleContract,
        "./data/abi/CryptoGames.json",
        event_derives(serde::Deserialize, serde::Serialize)
    );

    let contract = SimpleContract::new(contract_addr, client.clone());
    let filter = contract.play_filter().filter;

    let mut stream = client.provider().watch(&filter).await?.stream();
    while let Some(log) = stream.next().await {
        game.lock().unwrap().ticket_bought(format!("{:#x}", H160::from(log.topics[1])));
    }

    Ok(())
}