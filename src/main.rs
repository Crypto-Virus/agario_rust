
use dotenv;
use tokio::net::TcpListener;

use agario_rust::server;
use agario_rust::config;

const DEV: bool = true;

#[tokio::main]
async fn main() -> agario_rust::Result<()> {

    if DEV {
        dotenv::from_filename("dev.env").ok();
    } else {
        dotenv::dotenv().ok();
    }

    let config = config::load_config();
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", config.ws_port)).await?;
    server::run(config, listener).await;
    Ok(())
}



