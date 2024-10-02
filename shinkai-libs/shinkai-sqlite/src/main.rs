use shinkai_sqlite::{SqliteManager, SqliteLogger, LogEntry, Tool, WorkflowStep, WorkflowOperation, LogStatus};
use std::time::Instant;
use serde_json::json;
use chrono::Utc;

fn main() -> rusqlite::Result<()> {
    let manager = SqliteManager::new("example.db")?;
    let logger = SqliteLogger::new(&manager)?;

    // Add a tool
    let tool = Tool {
        id: 0,
        name: "MyProcess".to_string(),
        tool_type: "Workflow".to_string(),
        tool_router_key: Some("workflow_router".to_string()),
        instructions: Some("workflow MyProcess v0.1 { ... }".to_string()),
    };
    let tool_id = logger.add_tool(&tool)?;

    // Log a general (non-workflow) entry
    let general_log = LogEntry {
        id: Some(0),
        message_id: 1,
        tool_id,
        subprocess: None,
        parent_id: None,
        execution_order: 1,
        input: json!({"user_input": "Hello, world!"}),
        duration: Some(0.1),
        result: json!({"response": "Greetings!"}),
        status: LogStatus::Success,
        error_message: None,
        timestamp: Utc::now().to_rfc3339(),
        log_type: "general".to_string(),
        additional_info: None,
    };
    logger.add_log(&general_log)?;

    // Create a sample workflow
    let workflow = vec![
        WorkflowStep {
            name: "Initialize".to_string(),
            operations: vec![
                WorkflowOperation::RegisterOperation {
                    register: "$R1".to_string(),
                    value: "Create an outline for a blog post about the topic of the user's message ".to_string(),
                },
                WorkflowOperation::RegisterOperation {
                    register: "$R2".to_string(),
                    value: "\n separate the sections using a comma e.g. red,green,blue".to_string(),
                },
                WorkflowOperation::FunctionCall {
                    name: "concat".to_string(),
                    args: vec!["$R1".to_string(), "$R0".to_string()],
                },
                WorkflowOperation::FunctionCall {
                    name: "concat".to_string(),
                    args: vec!["$R3".to_string(), "$R2".to_string()],
                },
            ],
        },
    ];

    // Log the workflow execution
    logger.log_workflow_execution(1, tool_id, &workflow)?;

    // Benchmark for reading logs
    let start_read = Instant::now();
    let logs = logger.get_logs(Some(1), Some(tool_id), None)?;
    for log in logs.iter() {
        println!("{:?}", log);
    }
    let duration_read = start_read.elapsed();
    println!("Time taken to read logs: {:?}", duration_read);

    Ok(())
}