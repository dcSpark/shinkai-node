// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use shinkai_node;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_shinkai_node() -> String {
    eprintln!("Starting shinkai node");
    shinkai_node::run_node().await;
    "OK".to_string()
}

fn main() {
    dotenv::dotenv().ok();

    // Print environment variables
    for (key, value) in std::env::vars() {
        println!("{}: {}", key, value);
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, start_shinkai_node])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
