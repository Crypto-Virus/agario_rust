use std::env;

pub struct Config {
    pub ws_port: i32,
    pub secret_key: String,
    pub fee_manager_address: String,
    pub game_pool_address: String,
    pub multiplier: i32,
    pub no_entry_fee: bool,
}

pub fn load_config() -> Config {
    let ws_port = env::var("WS_PORT")
        .expect("Missing WS_PORT env variable!")
        .parse()
        .expect("Could not parse WS_PORT to integer");
    let secret_key = env::var("SECRET_KEY").expect("Missing SECRET_KEY env variable!");
    let fee_manager_address = env::var("FEE_MANAGER_ADDRESS").expect("Missing FEE_MANAGER_ADDRESS env variable!");
    let game_pool_address = env::var("GAME_POOL_ADDRESS").expect("Missing GAME_POOL_ADDRESS env variable!");
    let multiplier = env::var("MULTIPLIER")
        .expect("Missing MULTIPLIER env variable!")
        .parse()
        .expect("Could not parse MULTIPLIER to integer");
    let no_entry_fee = env::var("NO_ENTRY_FEE")
        .expect("Missing NO_ENTRY_FEE env variable!")
        .parse()
        .expect("Could not parse NO_ENTRY_FEE to bool");

    Config {
        ws_port,
        secret_key,
        fee_manager_address,
        game_pool_address,
        multiplier,
        no_entry_fee,
    }

}