use shinkai_sqlite::{LogTree, SqliteLogger, SqliteManager, Tool, WorkflowOperation, WorkflowStep};
use std::sync::Arc;

#[tokio::main]
async fn main() -> rusqlite::Result<()> {
    let manager = SqliteManager::new("example.db")?;
    let logger = SqliteLogger::new(Arc::new(manager))?;

    // Add a tool
    let tool = Tool {
        id: 0,
        name: "MyProcess".to_string(),
        tool_type: "Workflow".to_string(),
        tool_router_key: Some("workflow_router".to_string()),
        instructions: Some("workflow MyProcess v0.1 { ... }".to_string()),
    };
    let tool_id = logger.add_tool(&tool)?;

    // Create a sample workflow
    let workflow = vec![WorkflowStep {
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
    }];

    // Log the workflow execution
    let message_id = "1".to_string(); // Change this to a String
    logger.log_workflow_execution(message_id.clone(), tool_id.to_string(), &workflow)?;

    // Get all log IDs for this message
    let log_ids = logger.get_log_ids_for_message(&message_id)?;

    // Fetch and display the log tree for each log ID
    for log_id in log_ids {
        let log_tree = logger.get_log_tree(log_id).await?;
        println!("Log tree for log ID {}:", log_id);
        print_log_tree(&log_tree, 0);
        println!(); // Add a blank line between trees
    }

    Ok(())
}

fn print_log_tree(tree: &LogTree, depth: usize) {
    let indent = "  ".repeat(depth);
    println!(
        "{}Log ID: {}, Type: {}",
        indent,
        tree.log.id.unwrap(),
        tree.log.log_type
    );
    for child in &tree.children {
        print_log_tree(child, depth + 1);
    }
}
