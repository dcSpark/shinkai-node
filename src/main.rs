// main.rs
#![recursion_limit = "256"]
mod agent;
mod cron_tasks;
mod crypto_identities;
mod db;
mod managers;
mod network;
mod payments;
mod planner;
mod runner;
mod schemas;
mod tools;
mod utils;
mod vector_fs;

use runner::{initialize_node, run_node_tasks};

#[tokio::main]
pub async fn main() {
    #[cfg(feature = "console")]
    {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tokio_console=debug".to_string());
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(filter))
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::console::layer())
            .init();
        eprintln!("tokio-console is enabled");
    }

    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3, result.4).await;
}
