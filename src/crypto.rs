
use std::{
    str::FromStr,
    sync::Arc,
};
use ethers::prelude::*;
use ethers::core::k256::ecdsa::SigningKey;


use crate::Game;

pub async fn entry_fee_paid_event_listener(
    fee_manager_addr: String,
    game_pool_addr: String,
    client: Arc<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>,
    game: Game
) -> anyhow::Result<()> {

    let fee_manager_addr = H160::from_str(&fee_manager_addr).expect("Invalid fee manager address format");
    let game_pool_addr = H160::from_str(&game_pool_addr).expect("Invalid game pool address format");

    abigen!(
        SimpleContract,
        "./data/abi/FeeManager.json",
        event_derives(serde::Deserialize, serde::Serialize)
    );

    let contract = SimpleContract::new(fee_manager_addr, client.clone());
    let filter = contract.entry_fee_paid_filter().filter;

    let mut stream = client.provider().watch(&filter).await?.stream();
    while let Some(log) = stream.next().await {
        if game_pool_addr == H160::from(log.topics[2]) {
            game.lock().unwrap().ticket_bought(format!("{:#x}", H160::from(log.topics[1])));

        }
    }

    Ok(())
}