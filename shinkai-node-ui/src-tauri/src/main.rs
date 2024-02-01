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
use shinkai_node::network::Node;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Weak;
use tauri::async_runtime::Mutex;
use tauri::utils::platform::resource_dir;
use tauri::Manager;
use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, SystemTray, SystemTrayEvent, SystemTrayMenu};
use tokio::sync::broadcast;
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
}

impl NodeController {
    async fn send_command(&self, command: NodeCommand) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands.send(command).await.map_err(|e| Box::new(e) as _)
    }
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn get_settings() -> std::collections::HashMap<String, String> {
    let settings = SETTINGS.lock().await;
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
    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
        let _ = controller.send_command(NodeCommand::Shutdown).await;

        // Abort tasks using abort handles
        let mut node_tasks = NODE_TASKS.lock().await;
        if let Some((api_server_handle, node_task_handle, ws_server_handle)) = node_tasks.take() {
            node_task_handle.abort();
            api_server_handle.abort();
            ws_server_handle.abort();
        }
        "Node shutdown command sent".to_string()
    } else {
        "NodeController is not initialized".to_string()
    }
}

#[tauri::command]
async fn check_node_health() -> String {
    // eprintln!("Checking node health");
    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
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
        "NodeController is not initialized".to_string()
    }
}

async fn initialize_node() -> Result<
    (
        async_channel::Sender<NodeCommand>,
        JoinHandle<()>,
        JoinHandle<()>,
        JoinHandle<()>,
        Weak<Mutex<Node>>,
    ),
    String,
> {
    match shinkai_node::tauri_initialize_node().await {
        Ok((node_local_commands, api_server, node_task, ws_server, node)) => {
            let controller = NodeController {
                commands: node_local_commands.clone(),
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

            Ok((node_local_commands, api_server, node_task, ws_server, node))
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
        Ok((_, api_server, node_task, ws_server, node)) => {
            match shinkai_node::run_node_tasks(api_server, node_task, ws_server, node).await {
                Ok(_) => "Finished".to_string(),
                Err(e) => {
                    format!("Failed to run node tasks: {}", e)
                }
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
fn save_settings(settings: std::collections::HashMap<String, String>) -> Result<(), String> {
    let toml = toml::to_string(&settings).map_err(|e| e.to_string())?;
    let mut file = File::create("Settings.toml").map_err(|e| e.to_string())?;
    file.write_all(toml.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn stop_node_and_delete_storage() -> String {
    eprintln!("Stopping node and deleting storage");
    let node_controller = NODE_CONTROLLER.lock().await;
    if let Some(controller) = &*node_controller {
        // Send shutdown command to node
        let _ = controller.send_command(NodeCommand::Shutdown).await;

        // Abort tasks using abort handles
        let mut node_tasks = NODE_TASKS.lock().await;
        if let Some((api_server_handle, node_task_handle, ws_server_handle)) = node_tasks.take() {
            node_task_handle.abort();
            api_server_handle.abort();
            ws_server_handle.abort();
        }

        // Retrieve storage db path from settings or fallback to default
        let settings = SETTINGS.lock().await;
        let storage_db_path = settings
            .get_str("NODE_STORAGE_PATH")
            .unwrap_or_else(|_| "storage".to_string());

        match fs::remove_dir_all(&storage_db_path) {
            Ok(_) => eprintln!("Storage successfully deleted at {}", storage_db_path),
            Err(e) => eprintln!("Failed to delete storage at {}: {}", storage_db_path, e),
        }

        "Node stopped and storage deleted".to_string()
    } else {
        "NodeController is not initialized".to_string()
    }
}

fn load_settings(settings_file_path: String) {
    // Load settings from a TOML

    let mut settings = tauri::async_runtime::block_on(SETTINGS.lock());
    if let Err(e) = settings.merge(config::File::with_name(&settings_file_path).required(true)) {
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

fn main() {
    // Tray Code
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("show", "Show App"))
        // .add_item(CustomMenuItem::new("show", "Restart Node"))
        // .add_native_item(SystemTrayMenu::Separator)
        .add_item(CustomMenuItem::new("quit", "Quit"));

    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_shinkai_node,
            get_settings,
            check_node_health,
            stop_shinkai_node,
            stop_node_and_delete_storage,
            save_settings
        ])
        // This is the App menu
        // Update it to show:
        // - About
        // - Quit
        .menu(
            tauri::Menu::new().add_submenu(tauri::Submenu::new(
                "Shinkai",
                tauri::Menu::new()
                    .add_item(tauri::CustomMenuItem::new("start_node", "Start Node"))
                    .add_item(tauri::CustomMenuItem::new("stop_node", "Stop Node"))
                    .add_item(tauri::CustomMenuItem::new("check_health", "Check Node Health"))
                    .add_item(tauri::CustomMenuItem::new("get_settings", "Get Settings"))
                    .add_native_item(tauri::MenuItem::Separator)
                    .add_native_item(tauri::MenuItem::Quit),
            )),
        )
        .setup(|app| {
            let resource_path = app
                .path_resolver()
                .resource_dir()
                .expect("Failed to get resource directory");
            let settings_file_path = resource_path.join("Settings.toml");

            // Convert PathBuf to String
            let settings_file_path_str = settings_file_path
                .to_str()
                .expect("Path contains invalid Unicode")
                .to_owned();

            // Now you can pass the String to load_settings
            load_settings(settings_file_path_str);

            let window = app.get_window("main").unwrap();
            let icon_bytes = include_bytes!("../icons/icon.ico").to_vec();
            let icon = tauri::Icon::Raw(icon_bytes);
            window.set_icon(icon).expect("Failed to set icon");
            Ok(())
        })
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
