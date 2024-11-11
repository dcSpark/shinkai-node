// use serde_json::Value;
// use shinkai_dsl::dsl_schemas::{Action, Param, Step, StepBody, Workflow}; // Import necessary structs
// use shinkai_sqlite::{logger::{WorkflowLogEntry, WorkflowLogEntryStatus}, LogTree, SqliteLogger, SqliteManager};
// use std::{collections::VecDeque, fs, path::Path, sync::Arc};
// use tokio::sync::RwLock; // Use tokio's RwLock for async compatibility
// use chrono::Utc;

// #[tokio::main]
// async fn main() -> rusqlite::Result<()> {
//     // Remove the database file if it exists
//     let db_path = "example.db";
//     if Path::new(db_path).exists() {
//         fs::remove_file(db_path).expect("Failed to remove database file");
//     }

//     let manager = SqliteManager::new("example.db")?;
//     let logger = SqliteLogger::new(Arc::new(manager))?;

//     // Create a workflow
//     let workflow = Workflow {
//         name: "MyProcess".to_string(),
//         version: "v0.1".to_string(),
//         author: "nico@@shinkai.com".to_string(),
//         raw: "workflow MyProcess v0.1 { step myStep { command(\"param1\", \"param2\") } }".to_string(),
//         steps: vec![Step {
//             name: "myStep".to_string(),
//             body: vec![StepBody::Action(Action::Command {
//                 command: "command".to_string(),
//                 params: vec![Param::String("param1".to_string()), Param::String("param2".to_string())],
//             })],
//         }],
//         sticky: false,
//         description: Some("A simple workflow example.".to_string()),
//     };

//     // Print the current directory
//     let current_dir = std::env::current_dir().expect("Failed to get current directory");
//     println!("Current directory: {:?}", current_dir);

//     // Read logs from logs_main.json
//     let log_content = fs::read_to_string("./shinkai-libs/shinkai-sqlite/files/logs_main.json")
//         .expect("Failed to read logs_main.json");

//     // Parse the log_content JSON
//     let parsed_logs: Value = serde_json::from_str(&log_content).expect("Failed to parse log content");

//     // Create a VecDeque to store logs
//     let logs = Arc::new(RwLock::new(VecDeque::new()));

//     // Insert each log entry into the VecDeque
//     if let Some(log_entries) = parsed_logs.as_array() {
//         let mut logs_write = logs.write().await; // Use await with tokio's RwLock
//         for entry in log_entries {
//             if let (Some(subprocess), Some(additional_info), Some(timestamp), Some(status)) = (
//                 entry.get("subprocess").and_then(Value::as_str),
//                 entry.get("additional_info").and_then(Value::as_str),
//                 entry.get("timestamp").and_then(Value::as_str),
//                 entry.get("status").and_then(|s| s.get("Success").and_then(Value::as_str)),
//             ) {
//                 logs_write.push_back(WorkflowLogEntry {
//                     subprocess: Some(subprocess.to_string()),
//                     input: entry.get("input").and_then(Value::as_str).map(|s| s.to_string()),
//                     additional_info: additional_info.to_string(),
//                     timestamp: timestamp.parse().unwrap_or_else(|_| Utc::now()), // Parse timestamp or use current time
//                     status: WorkflowLogEntryStatus::Success(status.to_string()),
//                     result: entry.get("result").and_then(Value::as_str).map(|s| s.to_string()),
//                 });
//             }
//         }
//     }

//     eprintln!("Before logging");
//     // Log the workflow execution
//     let message_id = "550e8400-e29b-41d4-a716-446655440000".to_string();
//     logger
//         .log_workflow_execution(message_id.clone(), workflow, logs.clone())
//         .await?;

//     // Get all log IDs for this message
//     let log_ids = logger.get_log_ids_for_message(&message_id)?;

//     // Fetch and display the log tree for each log ID
//     eprintln!("Before getting log tree");
//     for log_id in log_ids {
//         let log_tree = logger.get_log_tree(log_id).await?;
//         println!("Log tree for log ID {}:", log_id);
//         print_log_tree(&log_tree, 0);
//         println!(); // Add a blank line between trees
//     }

//     Ok(())
// }

// fn print_log_tree(tree: &LogTree, depth: usize) {
//     let indent = "  ".repeat(depth);
//     let result_preview = match &tree.log.result {
//         Value::String(s) => s.chars().take(20).collect::<String>(),
//         _ => "Non-string result".to_string(),
//     };
//     println!(
//         "{}Log ID: {}, Type: {}, Result: {:?}, Status: {}, Timestamp: {}",
//         indent,
//         tree.log.id.unwrap(),
//         tree.log.log_type,
//         result_preview,     // Print only the first 20 characters of the result
//         tree.log.status,    // Assuming status is a field in your log structure
//         tree.log.timestamp  // Assuming timestamp is a field in your log structure
//     );
//     for child in &tree.children {
//         print_log_tree(child, depth + 1);
//     }
// }
