
use std::{
    env,
    str::FromStr,
    sync::Arc,
};
use ethers::prelude::*;
use ethers::core::k256::ecdsa::SigningKey;


use crate::Game;

pub async fn entry_fee_paid_event_listener(
    client: Arc<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>,
    game: Game
) -> anyhow::Result<()> {

    let fee_manager_addr = env::var("FEE_MANAGER_ADDRESS").unwrap();
    let fee_manager_addr = H160::from_str(&fee_manager_addr).unwrap();

    let game_pool_addr = env::var("GAME_POOL_ADDRESS").unwrap();
    let game_pool_addr = H160::from_str(&game_pool_addr).unwrap();

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