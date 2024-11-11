// main.rs
#![recursion_limit = "512"]
mod llm_provider;
mod cron_tasks;
mod managers;
mod network;
mod runner;
mod utils;
mod wallet;
mod tools;

use runner::{initialize_node, run_node_tasks};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;

#[cfg(feature = "console")]
use console_subscriber;


#[tokio::main]
pub async fn main() {
    init_default_tracing();
    #[cfg(feature = "console")]
    {
        console_subscriber::init();
        eprintln!("> tokio-console is enabled");
    }

    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3).await;
}
