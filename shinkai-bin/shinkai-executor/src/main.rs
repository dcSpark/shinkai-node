use clap::Parser;
use shinkai_executor::{
    api,
    cli::{Cli, CliArgs},
};
use std::{net::SocketAddr, str::FromStr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    match args.cmd {
        // Run CLI command
        Some(cmd) => Cli::run_cli_command(cmd).await?,
        // Run API server
        None => {
            let address = SocketAddr::from_str(&args.address).unwrap();
            api::run_api(address).await?;
        }
    }

    Ok(())
}
