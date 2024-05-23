use clap::Parser;
use shinkai_side_executor::api;
use std::{net::SocketAddr, str::FromStr};

const DEFAULT_ADDRESS: &str = "0.0.0.0:8090";

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(default_value = DEFAULT_ADDRESS, short, long)]
    address: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let address = SocketAddr::from_str(&args.address).unwrap();

    api::api_handlers::run_api(address).await?;

    Ok(())
}
