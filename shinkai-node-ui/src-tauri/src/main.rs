// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use async_channel::Sender;
use config::Config;
use config::Source;
use once_cell::sync::Lazy;
use shinkai_node;
use shinkai_node::network::node::NodeCommand;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use std::env;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tauri::async_runtime::Mutex;
use tokio::sync::oneshot;
use toml;

lazy_static! {
    static ref SETTINGS: Mutex<Config> = Mutex::new(Config::default());
}

static NODE_CONTROLLER: Lazy<Arc<Mutex<Option<NodeController>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
static NODE_TASKS: Lazy<Arc<Mutex<Option<(JoinHandle<()>, JoinHandle<()>, JoinHandle<()>, broadcast::Sender<()>)>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

struct NodeController {
    commands: Sender<NodeCommand>,
}

impl NodeController {
    async fn send_command(&self, command: NodeCommand) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands.send(command).await.map_err(|e| Box::new(e) as _)
    }
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_settings() -> std::collections::HashMap<String, String> {
    eprintln!("Getting settings");
    let settings = SETTINGS.lock().await;
    eprintln!("after lock");
    let settings_map = settings
        .collect()
        .unwrap()
        .into_iter()
        .filter_map(|(key, value)| value.into_str().ok().map(|v| (key, v)))
        .collect::<std::collections::HashMap<_, _>>();

    println!("settings_map: {:?}", settings_map);

    settings_map
}

#[tauri::command]
async fn stop_shinkai_node() -> String {
    eprintln!("Stopping shinkai node");
    let mut node_tasks = NODE_TASKS.lock().await;
    if let Some((api_server, node_task, ws_server, tx)) = node_tasks.take() {
        api_server.abort();
        node_task.abort();
        ws_server.abort();
        match tx.send(()) {
            Ok(_) => "Node shutdown command sent".to_string(),
            Err(e) => format!("Failed to send shutdown command: {}", e),
        }
    } else {
        eprintln!("Node tasks are not running");
        "Node tasks are not running".to_string()
    }
}

#[tauri::command]
async fn check_node_health() -> String {
    // eprintln!("Checking node health");
    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        match controller.send_command(NodeCommand::IsPristine { res: res_sender }).await {
            Ok(_) => match res_receiver.recv().await {
                Ok(is_pristine) => {
                    // eprintln!("is_pristine: {}", is_pristine);
                    if is_pristine {
                        "Node is pristine".to_string()
                    } else {
                        "Node is not pristine".to_string()
                    }
                },
                Err(_) => "Failed to receive response".to_string(),
            },
            Err(_) => "Failed to send command".to_string(),
        }
    } else {
        eprintln!("NodeController is not initialized");
        "NodeController is not initialized".to_string()
    }
}

async fn initialize_node() -> Result<(async_channel::Sender<NodeCommand>, JoinHandle<()>, JoinHandle<()>, JoinHandle<()>), String> {
    match shinkai_node::tauri_initialize_node().await {
        Ok((node_local_commands, api_server, node_task, ws_server)) => {
            let controller = NodeController {
                commands: node_local_commands.clone(),
            };
            eprintln!("\n\n Initializing node controller");
            let mut node_controller = NODE_CONTROLLER.lock().await;
            *node_controller = Some(controller);
            eprintln!("\n\n Node initialized successfully");
            Ok((node_local_commands, api_server, node_task, ws_server))
        }
        Err(e) => {
            eprintln!("Failed to initialize node: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn start_shinkai_node() -> String {
    eprintln!("Starting shinkai node");
    match initialize_node().await {
        Ok((node_local_commands, api_server, node_task, ws_server)) => {
            let (tx, _) = broadcast::channel(1);
            let rx1 = tx.subscribe();
            let rx2 = tx.subscribe();
            let rx3 = tx.subscribe();

            let api_server = tokio::spawn(run_with_cancellation(api_server, rx1));
            let node_task = tokio::spawn(run_with_cancellation(node_task, rx2));
            let ws_server = tokio::spawn(run_with_cancellation(ws_server, rx3));

            let mut node_tasks = NODE_TASKS.lock().await;
            *node_tasks = Some((api_server, node_task, ws_server, tx));

            "".to_string()
        }
        Err(e) => e,
    }
}

async fn run_with_cancellation(task: JoinHandle<()>, mut rx: broadcast::Receiver<()>) {
    tokio::select! {
        _ = task => {},
        _ = rx.recv() => {},
    }
}

#[tauri::command]
fn save_settings(settings: std::collections::HashMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let toml = toml::to_string(&settings)?;
    let mut file = File::create("Settings.toml")?;
    file.write_all(toml.as_bytes())?;
    Ok(())
}

fn main() {
    // Load settings from a TOML
    {
        let mut settings = tauri::async_runtime::block_on(SETTINGS.lock());
        if let Err(e) = settings.merge(config::File::with_name("Settings.toml").required(true)) {
            eprintln!("Failed to merge settings: {}", e);
        }

        // Set environment variables from settings
        for (key, value) in settings.collect().unwrap().iter() {
            // Use the correct method to iterate
            if let Some(val) = value.clone().into_str().ok() {
                // Clone value before calling into_str
                env::set_var(key, val);
            }
        }
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            greet,
            start_shinkai_node,
            get_settings,
            check_node_health,
            stop_shinkai_node
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
