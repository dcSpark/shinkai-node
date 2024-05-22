use std::{net::SocketAddr, str::FromStr};

use shinkai_side_executor::api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = SocketAddr::from_str("127.0.0.1:8090").unwrap();

    api::api_handlers::run_api(address).await?;

    Ok(())
}
