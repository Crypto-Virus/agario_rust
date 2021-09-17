
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

    let contract_addr = env::var("GAME_POOL_ADDRESS").unwrap();
    let contract_addr = H160::from_str(&contract_addr).unwrap();

    abigen!(
        SimpleContract,
        "./data/abi/GamePool.json",
        event_derives(serde::Deserialize, serde::Serialize)
    );

    let contract = SimpleContract::new(contract_addr, client.clone());
    let filter = contract.entry_fee_paid_filter().filter;

    let mut stream = client.provider().watch(&filter).await?.stream();
    while let Some(log) = stream.next().await {
        game.lock().unwrap().ticket_bought(format!("{:#x}", H160::from(log.topics[1])));
    }

    Ok(())
}