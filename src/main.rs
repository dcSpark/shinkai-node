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
mod schemas;
mod tools;
mod utils;
mod vector_fs;
mod runner;

use runner::{initialize_node, run_node_tasks};

use std::thread;
use std::time::Duration;
use parking_lot::deadlock;

extern crate core_affinity;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
// Create a background thread which checks for deadlocks every 10s
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            let deadlocks = deadlock::check_deadlock();
            if deadlocks.is_empty() {
                println!("No deadlocks found");
                continue;
            }

            println!("{} deadlocks detected", deadlocks.len());
            for (i, threads) in deadlocks.iter().enumerate() {
                println!("Deadlock #{}", i);
                for t in threads {
                    println!("Thread Id {:#?}", t.thread_id());
                    println!("{:#?}", t.backtrace());
                }
            }
        }
    });
    console_subscriber::init();
    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3, result.4).await;
}
