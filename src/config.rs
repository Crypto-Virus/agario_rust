use std::env;

pub struct Config {
    pub ws_port: i32,
    pub chain_id: u32,
    pub provider_http_url: String,
    pub provider_ws_url: String,
    pub secret_key: String,
    pub fee_manager_address: String,
    pub game_pool_address: String,
    pub multiplier: u32,
    pub no_entry_fee: bool,
}

pub fn load_config() -> Config {
    let ws_port = env::var("WS_PORT")
        .expect("Missing WS_PORT env variable!")
        .parse()
        .expect("Could not parse WS_PORT to integer");
    let chain_id = env::var("CHAIN_ID")
        .expect("Missing CHAIN_ID env variable!")
        .parse()
        .expect("Could not parse CHAIN_ID to unsigned integer");
    let provider_http_url = env::var("PROVIDER_HTTP_URL").expect("Missing PROVIPROVIDER_HTTP_URLDER_WS_URL env variable!");
    let provider_ws_url = env::var("PROVIDER_WS_URL").expect("Missing PROVIDER_WS_URL env variable!");
    let secret_key = env::var("SECRET_KEY").expect("Missing SECRET_KEY env variable!");
    let fee_manager_address = env::var("FEE_MANAGER_ADDRESS").expect("Missing FEE_MANAGER_ADDRESS env variable!");
    let game_pool_address = env::var("GAME_POOL_ADDRESS").expect("Missing GAME_POOL_ADDRESS env variable!");
    let multiplier = env::var("MULTIPLIER")
        .expect("Missing MULTIPLIER env variable!")
        .parse()
        .expect("Could not parse MULTIPLIER to unsigned integer");
    let no_entry_fee = env::var("NO_ENTRY_FEE")
        .expect("Missing NO_ENTRY_FEE env variable!")
        .parse()
        .expect("Could not parse NO_ENTRY_FEE to bool");

    Config {
        ws_port,
        chain_id,
        provider_http_url,
        provider_ws_url,
        secret_key,
        fee_manager_address,
        game_pool_address,
        multiplier,
        no_entry_fee,
    }

}