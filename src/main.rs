// main.rs
mod agent;
mod cron_tasks;
mod crypto_identities;
mod db;
mod managers;
mod network;
mod payments;
mod planner;
mod schemas;
mod tools;
mod utils;
mod vector_fs;
mod runner;

use shinkai_node::run_node;

#[tokio::main]
pub async fn main() {
    run_node().await.unwrap();
}
