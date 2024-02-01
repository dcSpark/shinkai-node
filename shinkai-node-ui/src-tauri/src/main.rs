// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use async_channel::Sender;
use config::Config;
use config::Source;
use once_cell::sync::Lazy;
use shinkai_node;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::network::node;
use shinkai_node::network::node::NodeCommand;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tauri::async_runtime::Mutex;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use toml;

lazy_static! {
    static ref SETTINGS: Mutex<Config> = Mutex::new(Config::default());
}

static NODE_CONTROLLER: Lazy<Arc<Mutex<Option<NodeController>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
static NODE_TASKS: Lazy<
    Mutex<
        Option<(
            tokio::task::AbortHandle,
            tokio::task::AbortHandle,
            tokio::task::AbortHandle,
        )>,
    >,
> = Lazy::new(|| Mutex::new(None));

struct NodeController {
    commands: Sender<NodeCommand>,
    db_path: String,
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
    eprintln!("after lock");
    // eprintln!("after take");
    // api_server.abort();
    // node_task.abort();
    // ws_server.abort();
    // eprintln!("after aborts");

    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
        eprintln!("controller OK");

        // Abort tasks using abort handles
        let mut node_tasks = NODE_TASKS.lock().await;
        if let Some((api_server_handle, node_task_handle, ws_server_handle)) = node_tasks.take() {
            node_task_handle.abort();
            api_server_handle.abort();
            ws_server_handle.abort();
        }

        eprintln!("after aborts");

        eprintln!("db_path: {}", controller.db_path);
        // wait 1 second for tasks to finish
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = force_remove_db_lock(Path::new(controller.db_path.clone().as_str()));
        // let shinkai_db = controller.shinkai_db.clone();

        // // Set needs_reset to true
        // shinkai_db.lock().await.set_needs_reset().unwrap();
        // eprintln!("after set_needs_reset");
        // let value = shinkai_db.lock().await.read_needs_reset().unwrap();
        // eprintln!("value: {}", value);

        "Node shutdown command sent".to_string()
    } else {
        eprintln!("NodeController is not initialized");
        "NodeController is not initialized".to_string()
    }
}

#[tauri::command]
async fn check_node_health() -> String {
    // eprintln!("Checking node health");
    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
        eprintln!("check_node_health> controller OK");
        let (res_sender, res_receiver) = async_channel::bounded(1);
        match controller
            .send_command(NodeCommand::IsPristine { res: res_sender })
            .await
        {
            Ok(_) => match res_receiver.recv().await {
                Ok(is_pristine) => {
                    // eprintln!("is_pristine: {}", is_pristine);
                    if is_pristine {
                        "Node is pristine".to_string()
                    } else {
                        "Node is not pristine".to_string()
                    }
                }
                Err(_) => "Failed to receive response".to_string(),
            },
            Err(_) => "Failed to send command".to_string(),
        }
    } else {
        eprintln!("NodeController is not initialized");
        "NodeController is not initialized".to_string()
    }
}

async fn initialize_node() -> Result<
    (
        async_channel::Sender<NodeCommand>,
        JoinHandle<()>,
        JoinHandle<()>,
        JoinHandle<()>,
    ),
    String,
> {
    match shinkai_node::tauri_initialize_node().await {
        Ok((node_local_commands, api_server, node_task, ws_server, main_db_path)) => {
            let controller = NodeController {
                commands: node_local_commands.clone(),
                db_path: main_db_path.clone(),
            };

            eprintln!("\n\n Initializing node controller");
            let mut node_controller = NODE_CONTROLLER.lock().await;
            *node_controller = Some(controller);
            eprintln!("\n\n Node initialized successfully");

            let mut node_tasks = NODE_TASKS.lock().await;
            *node_tasks = Some((
                api_server.abort_handle(),
                node_task.abort_handle(),
                ws_server.abort_handle(),
            ));

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
        Ok((_, api_server, node_task, ws_server)) => {
            match shinkai_node::run_node_tasks(api_server, node_task, ws_server).await {
                Ok(_) => "Finished".to_string(),
                Err(e) => {
                    eprintln!("Failed to run node tasks: {}", e);
                    format!("Failed to run node tasks: {}", e)
                },
            }
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

fn force_remove_db_lock(db_path: &Path) -> std::io::Result<()> {
    let lock_file_path = db_path.join("LOCK");
    if lock_file_path.exists() {
        eprintln!("Removing LOCK file");
        fs::remove_file(lock_file_path)?;
    } else {
        eprintln!("LOCK file does not exist");
    }
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
