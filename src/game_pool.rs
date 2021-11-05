
use std::{
    convert::TryFrom,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use ethers::{abi::{ParamType, Token, decode}, prelude::*};
use ethers::core::k256::ecdsa::SigningKey;


abigen!(
    GamePoolContract,
    "./data/abi/GamePool.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

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

    // abigen!(
    //     GamePoolContract,
    //     "./data/abi/GamePool.json",
    //     event_derives(serde::Deserialize, serde::Serialize)
    // );

    let contract = GamePoolContract::new(
        game_pool_addr,
        provider.clone(),
    );

    // get initial rewards available in server
    let amount = contract.game_pool_rewards().call().await.expect("Failed to get gamePoolRewards");
    game.lock().unwrap().add_rewards(amount.as_u128());

    // watch for rewards added events
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



pub struct Winner {
   address: H160,
   amount: U256,
}

impl Winner {
    pub fn new(address: &str, amount: u128) -> Self {
        Winner {
            address: H160::from_str(address).unwrap(),
            amount: U256::from(amount),
        }
    }
}


pub fn winner_listener(
    address: &str,
    client: Arc<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>,
) -> tokio::sync::mpsc::UnboundedSender<Winner> {

    let address = H160::from_str(address).expect("Game pool address is invalid");
    let contract = GamePoolContract::new(address, client);

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Winner>();
    let mut stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

    tokio::spawn(async move {
        while let Some(winner) = stream.next().await {

            contract.award_winner(
                winner.address,
                winner.amount
            ).legacy().send().await.unwrap().await;
        }
    });

    tx
}