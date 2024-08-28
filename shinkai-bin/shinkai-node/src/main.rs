// main.rs
#![recursion_limit = "512"]
mod llm_provider;
mod cron_tasks;
mod db;
mod managers;
mod network;
mod planner;
mod runner;
mod schemas;
mod tools;
mod utils;
mod vector_fs;
mod welcome_files;
mod workflows;
mod lance_db;
mod wallet;

use runner::{initialize_node, run_node_tasks};

#[cfg(feature = "console")]
use console_subscriber;

#[tokio::main]
pub async fn main() {
    #[cfg(feature = "console")]
    {
        console_subscriber::init();
        eprintln!("> tokio-console is enabled");
    }

    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3).await;
}
