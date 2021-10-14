
use std::{
    convert::TryFrom,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use ethers::{abi::{ParamType, Token, decode}, prelude::*};

pub async fn game_pool_reward_added_listener(
    provider_http_url: String,
    game: crate::Game,
    game_pool_addr: String
) -> anyhow::Result<()> {

    let game_pool_addr = H160::from_str(&game_pool_addr).expect("Invalid game pool address format");

    let provider = Provider::<Http>::try_from(provider_http_url)
        .expect("Invalid http rpc endpoint")
        .interval(Duration::from_secs(2u64));
    let provider = Arc::new(provider);

    abigen!(
        GamePoolContract,
        "./data/abi/GamePool.json",
        event_derives(serde::Deserialize, serde::Serialize)
    );

    let contract = GamePoolContract::new(
        game_pool_addr,
        provider.clone(),
    );
    let filter = contract.rewards_added_filter().filter;

    let mut stream = provider.watch(&filter).await?.stream();
    while let Some(log) = stream.next().await {
        let bytes = log.data;
        if let Ok(tokens) = decode(&[ParamType::Uint(256)], bytes.as_ref()) {
            if let Token::Uint(value) = &tokens[0] {
                let amount = value.as_u128();
                game.lock().unwrap().add_rewards(amount);
            }
        }
    }

    Ok(())
}