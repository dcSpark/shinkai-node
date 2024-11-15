use std::sync::Arc;
use std::time::Instant;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::{api_v2::api_v2_handlers_tools::Language, node_api_router::APIError};

use shinkai_sqlite::{shinkai_tool_manager::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{
    argument::ToolOutputArg, deno_tools::DenoTool, error::ToolError, playground_tool::PlaygroundTool,
    shinkai_tool::ShinkaiTool,
};

use crate::{
    network::{node_error::NodeError, Node},
    tools::generate_tool_definitions,
};

impl Node {
    pub async fn v2_api_search_shinkai_tool(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
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

        // Perform the internal search using SqliteManager
        // TODO: implement something like BTS for tools
        match sqlite_manager.prompt_vector_search(&query, 5).await {
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

    pub async fn v2_api_list_all_shinkai_tools(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match sqlite_manager.get_all_tool_headers() {
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
        sqlite_manager: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        input_value: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the full tool from sqlite_manager
        let existing_tool = match sqlite_manager.get_tool_by_key(&tool_router_key) {
            Ok(tool) => tool,
            Err(SqliteManagerError::ToolNotFound(_)) => {
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
        let save_result = sqlite_manager.update_tool(merged_tool).await;

        match save_result {
            Ok(tool) => {
                let _ = res.send(Ok(tool)).await;
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

    pub async fn v2_api_add_shinkai_tool(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        bearer: String,
        new_tool: ShinkaiTool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new tool to the LanceShinkaiDb
        let save_result = sqlite_manager.add_tool(new_tool).await;

        match save_result {
            Ok(tool) => {
                let tool_key = tool.tool_router_key();
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
        sqlite_manager: Arc<SqliteManager>,
        bearer: String,
        payload: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database using get_tool_by_key
        match sqlite_manager.get_tool_by_key(&payload) {
            Ok(tool) => {
                let response = json!(tool);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(SqliteManagerError::ToolNotFound(_)) => {
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
                    message: format!("Failed to get tool: {:?}", err),
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

    pub async fn v2_api_set_playground_tool(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        bearer: String,
        payload: PlaygroundTool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // TODO: check that job_id exists

        // TODO: do i need this?
        // let header_code = generate_tool_definitions(Language::Typescript, sqlite_manager.clone(), false)
        //     .await
        //     .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

        let mut updated_payload = payload.clone();

        // Create DenoTool
        let tool = DenoTool {
            toolkit_name: {
                let name = format!(
                    "{}_{}",
                    payload
                        .metadata
                        .name
                        .to_lowercase()
                        .replace(" ", "_")
                        .replace("-", "_")
                        .replace(":", "_"),
                    payload
                        .metadata
                        .author
                        .to_lowercase()
                        .replace(" ", "_")
                        .replace("-", "_")
                        .replace(":", "_")
                );
                // Use a regex to filter out unwanted characters
                let re = regex::Regex::new(r"[^a-z0-9_]").unwrap();
                re.replace_all(&name, "").to_string()
            },
            name: payload.metadata.name.clone(),
            author: payload.metadata.author.clone(),
            js_code: payload.code.clone(),
            config: payload.metadata.configurations.clone(),
            description: payload.metadata.description.clone(),
            keywords: payload.metadata.keywords.clone(),
            input_args: payload.metadata.parameters.clone(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: false, // TODO: maybe we want to add this as an option in the UI?
            embedding: None,
            result: payload.metadata.result,
        };

        let shinkai_tool = ShinkaiTool::Deno(tool, false); // Same as above
        updated_payload.tool_router_key = Some(shinkai_tool.tool_router_key());

        // Function to handle saving metadata and sending response
        async fn save_metadata_and_respond(
            db: &ShinkaiDB,
            res: &Sender<Result<Value, APIError>>,
            updated_payload: PlaygroundTool,
            tool: ShinkaiTool,
        ) -> Result<(), NodeError> {
            if let Err(err) = db.save_playground_tool(updated_payload) {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to save playground tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            match serde_json::to_value(&tool) {
                Ok(tool_json) => {
                    let _ = res.send(Ok(tool_json)).await;
                }
                Err(_) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to serialize tool to JSON".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
            }

            Ok(())
        }

        // Save the tool to the LanceShinkaiDb
        match sqlite_manager.add_tool(shinkai_tool.clone()).await {
            Ok(tool) => {
                save_metadata_and_respond(&db, &res, updated_payload, tool).await
            }
            Err(SqliteManagerError::ToolAlreadyExists(_)) => {
                // Tool already exists, update it instead
                match sqlite_manager.update_tool(shinkai_tool).await {
                    Ok(tool) => {
                        save_metadata_and_respond(&db, &res, updated_payload, tool).await
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update tool in LanceShinkaiDb: {}", err),
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
                    message: format!("Failed to add tool to SqliteManager: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // TODO: update to not use workflow
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
