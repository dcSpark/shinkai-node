// main.rs
#![recursion_limit = "512"]
mod cron_tasks;
mod llm_provider;
mod managers;
mod network;
mod runner;
mod tools;
mod utils;
mod wallet;

use runner::{initialize_node, run_node_tasks};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;

#[cfg(feature = "console")]
use console_subscriber;

#[tokio::main]
pub async fn main() {
    // Initialize crypto provider for rustls (required by ngrok)
    #[cfg(feature = "ngrok")]
    {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    // Initialize logging based on features
    #[cfg(feature = "console")]
    {
        // When using console subscriber, we don't need env_logger
        console_subscriber::init();
        eprintln!("> tokio-console is enabled");
    }
    #[cfg(not(feature = "console"))]
    {
        // When not using console subscriber, use the default logging setup
        env_logger::Builder::from_env(env_logger::Env::default())
            .format_timestamp_millis()
            .init();
        init_default_tracing();
    }

    println!("Starting Shinkai Node...");

    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3).await;
}
