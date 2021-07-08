
use tokio::net::TcpListener;

use agario_rust::server;

#[tokio::main]
async fn main() -> agario_rust::Result<()> {

    let port = 8080;
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await?;
    server::run(listener).await;
    Ok(())
}



