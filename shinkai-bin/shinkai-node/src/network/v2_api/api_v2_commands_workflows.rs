use std::sync::Arc;
use std::time::Instant;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APISetWorkflow;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIWorkflowKeyname;
use tokio::sync::Mutex;

use crate::lance_db::shinkai_lance_db::LanceShinkaiDb;
use crate::{
    db::ShinkaiDB,
    network::{node_api_router::APIError, node_error::NodeError, Node},
    tools::{shinkai_tool::ShinkaiTool, workflow_tool::WorkflowTool},
};

impl Node {
    pub async fn v2_api_search_workflows(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Start the timer
        let start_time = Instant::now();

        // Perform the internal search using LanceShinkaiDb
        match lance_db.lock().await.workflow_vector_search(&query, 5).await {
            Ok(workflows) => {
                let workflows_json = serde_json::to_value(workflows).map_err(|err| NodeError {
                    message: format!("Failed to serialize workflows: {}", err),
                })?;
                // Log the elapsed time if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    let result_count = workflows_json.as_array().map_or(0, |arr| arr.len());
                    println!("Time taken for workflow search: {:?}", elapsed_time);
                    println!("Number of workflow results: {}", result_count);
                }
                let _ = res.send(Ok(workflows_json)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to search workflows: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_search_shinkai_tool(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Start the timer
        let start_time = Instant::now();

        // Perform the internal search using LanceShinkaiDb
        match lance_db.lock().await.vector_search_all_tools(&query, 5, true).await {
            Ok(tools) => {
                let tools_json = serde_json::to_value(tools).map_err(|err| NodeError {
                    message: format!("Failed to serialize tools: {}", err),
                })?;
                // Log the elapsed time if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    let result_count = tools_json.as_array().map_or(0, |arr| arr.len());
                    println!("Time taken for tool search: {:?}", elapsed_time);
                    println!("Number of tool results: {}", result_count);
                }
                let _ = res.send(Ok(tools_json)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to search tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_set_workflow(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        payload: APISetWorkflow,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Create the workflow using the new method
        let workflow = match Workflow::new(payload.workflow_raw, payload.description) {
            Ok(workflow) => workflow,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to create workflow: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Create a WorkflowTool from the workflow
        let workflow_tool = WorkflowTool::new(workflow.clone());

        // Create a ShinkaiTool::Workflow
        let shinkai_tool = ShinkaiTool::Workflow(workflow_tool, true);

        // Save the workflow to the LanceShinkaiDb
        match lance_db.lock().await.set_tool(&shinkai_tool).await {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Workflow added to LanceShinkaiDb" });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add workflow to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_remove_workflow(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Generate the workflow key
        let workflow_key_str = payload.generate_key();

        // Remove the workflow from the LanceShinkaiDb
        match lance_db.lock().await.remove_tool(&workflow_key_str).await {
            Ok(_) => {
                let response = json!({ "message": "Workflow removed from database" });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove workflow from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_workflow_info(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>, // Updated to use LanceShinkaiDb
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Generate the workflow key
        let workflow_key_str = payload.generate_key();

        // Get the workflow from the database
        match lance_db.lock().await.get_tool(&workflow_key_str).await {
            Ok(Some(workflow)) => {
                let response = json!(workflow);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Workflow not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get workflow: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_all_workflows(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all workflows for the user
        match lance_db.lock().await.get_all_workflows().await {
            Ok(workflows) => {
                let response = json!(workflows);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list workflows: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_all_shinkai_tools(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match lance_db.lock().await.get_all_tools(true).await {
            Ok(tools) => {
                let response = json!(tools);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_set_shinkai_tool(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        tool_router_key: String,
        input_value: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the full tool from lance_db
        let existing_tool = {
            let lance_db_lock = lance_db.lock().await;
            match lance_db_lock.get_tool(&tool_router_key).await {
                Ok(Some(tool)) => tool.clone(),
                Ok(None) => {
                    let api_error = APIError {
                        code: StatusCode::NOT_FOUND.as_u16(),
                        error: "Not Found".to_string(),
                        message: "Tool not found in LanceShinkaiDb".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Convert existing_tool to Value
        let existing_tool_value = match serde_json::to_value(&existing_tool) {
            Ok(value) => value,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert existing tool to Value: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Merge existing_tool_value with input_value
        let merged_value = Self::merge_json(existing_tool_value, input_value);

        // Convert merged_value to ShinkaiTool
        let merged_tool: ShinkaiTool = match serde_json::from_value(merged_value) {
            Ok(tool) => tool,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert merged Value to ShinkaiTool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Save the tool to the LanceShinkaiDb
        let save_result = {
            let lance_db_lock = lance_db.lock().await;
            lance_db_lock.set_tool(&merged_tool).await
        };

        match save_result {
            Ok(_) => {
                // Fetch the updated tool from the database
                let updated_tool = {
                    let lance_db_lock = lance_db.lock().await;
                    lance_db_lock.get_tool(&tool_router_key).await
                };

                match updated_tool {
                    Ok(Some(tool)) => {
                        let _ = res.send(Ok(tool)).await;
                        Ok(())
                    }
                    Ok(None) => {
                        let api_error = APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "Tool not found in LanceShinkaiDb".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add tool to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_add_shinkai_tool(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        new_tool: ShinkaiTool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new tool to the LanceShinkaiDb
        let save_result = {
            let lance_db_lock = lance_db.lock().await;
            lance_db_lock.set_tool(&new_tool).await
        };

        match save_result {
            Ok(_) => {
                let tool_key = new_tool.tool_router_key();
                let response = json!({ "status": "success", "message": format!("Tool added with key: {}", tool_key) });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add tool to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_shinkai_tool(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        payload: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database
        match lance_db.lock().await.get_tool(&payload).await {
            Ok(Some(tool)) => {
                let response = json!(tool);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub fn merge_json(existing: Value, input: Value) -> Value {
        match (existing, input) {
            (Value::Object(mut existing_map), Value::Object(input_map)) => {
                for (key, input_value) in input_map {
                    let existing_value = existing_map.remove(&key).unwrap_or(Value::Null);
                    existing_map.insert(key, Self::merge_json(existing_value, input_value));
                }
                Value::Object(existing_map)
            }
            (Value::Array(mut existing_array), Value::Array(input_array)) => {
                for (i, input_value) in input_array.into_iter().enumerate() {
                    if i < existing_array.len() {
                        existing_array[i] = Self::merge_json(existing_array[i].take(), input_value);
                    } else {
                        existing_array.push(input_value);
                    }
                }
                Value::Array(existing_array)
            }
            (_, input) => input,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_json() {
        let existing_tool_value = json!({
            "content": [{
                "embedding": {
                    "id": "",
                    "vector": [0.1, 0.2, 0.3]
                },
                "workflow": {
                    "author": "@@official.shinkai",
                    "description": "Reviews in depth all the content to generate a summary.",
                    "name": "Extensive_summary",
                    "raw": "workflow Extensive_summary v0.1 { ... }",
                    "steps": [
                        {
                            "body": [
                                {
                                    "type": "composite",
                                    "value": [
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$PROMPT",
                                                "value": "Summarize this: "
                                            }
                                        },
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$EMBEDDINGS",
                                                "value": {
                                                    "args": [],
                                                    "name": "process_embeddings_in_job_scope"
                                                }
                                            }
                                        }
                                    ]
                                }
                            ],
                            "name": "Initialize"
                        },
                        {
                            "body": [
                                {
                                    "type": "registeroperation",
                                    "value": {
                                        "register": "$RESULT",
                                        "value": {
                                            "args": ["$PROMPT", "$EMBEDDINGS"],
                                            "name": "multi_inference"
                                        }
                                    }
                                }
                            ],
                            "name": "Summarize"
                        }
                    ],
                    "sticky": true,
                    "version": "v0.1"
                }
            }],
            "type": "Workflow"
        });

        let input_value = json!({
            "content": [{
                "workflow": {
                    "description": "New description"
                }
            }],
            "type": "Workflow"
        });

        let expected_merged_value = json!({
            "content": [{
                "embedding": {
                    "id": "",
                    "vector": [0.1, 0.2, 0.3]
                },
                "workflow": {
                    "author": "@@official.shinkai",
                    "description": "New description",
                    "name": "Extensive_summary",
                    "raw": "workflow Extensive_summary v0.1 { ... }",
                    "steps": [
                        {
                            "body": [
                                {
                                    "type": "composite",
                                    "value": [
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$PROMPT",
                                                "value": "Summarize this: "
                                            }
                                        },
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$EMBEDDINGS",
                                                "value": {
                                                    "args": [],
                                                    "name": "process_embeddings_in_job_scope"
                                                }
                                            }
                                        }
                                    ]
                                }
                            ],
                            "name": "Initialize"
                        },
                        {
                            "body": [
                                {
                                    "type": "registeroperation",
                                    "value": {
                                        "register": "$RESULT",
                                        "value": {
                                            "args": ["$PROMPT", "$EMBEDDINGS"],
                                            "name": "multi_inference"
                                        }
                                    }
                                }
                            ],
                            "name": "Summarize"
                        }
                    ],
                    "sticky": true,
                    "version": "v0.1"
                }
            }],
            "type": "Workflow"
        });

        let merged_value = Node::merge_json(existing_tool_value, input_value);
        assert_eq!(merged_value, expected_merged_value);
    }
}
