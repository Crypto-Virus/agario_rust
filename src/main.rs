
use dotenv;
use tokio::net::TcpListener;

use agario_rust::server;
use agario_rust::config;



#[tokio::main]
async fn main() -> agario_rust::Result<()> {
    dotenv::dotenv().ok();

    let config = config::load_config();
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", config.ws_port)).await?;
    server::run(config, listener).await;
    Ok(())
}



