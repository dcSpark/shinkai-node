use crate::{
    network::{node_error::NodeError, Node},
    utils::environment::NodeEnvironment,
};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{
        indexable_version::IndexableVersion, shinkai_name::ShinkaiName, tool_router_key::ToolRouterKey,
    },
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{
    shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use std::fs;

impl Node {
    pub async fn v2_api_search_shinkai_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        query: String,
        agent_or_llm: Option<String>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let sanitized_query = query.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");

        let allowed_tools = if let Some(agent_id) = agent_or_llm {
            match db.get_agent(&agent_id) {
                Ok(Some(agent)) => Some(agent.tools),
                Ok(None) | Err(_) => None,
            }
        } else {
            None
        };

        let start_time = std::time::Instant::now();

        let vector_start_time = std::time::Instant::now();

        let vector_search_result = if let Some(tools) = allowed_tools {
            let embedding = db
                .generate_embeddings(&sanitized_query)
                .await
                .map_err(|e| shinkai_tools_primitives::tools::error::ToolError::DatabaseError(e.to_string()))?;

            let tool_names = tools
                .iter()
                .map(|tool| tool.to_string_without_version())
                .collect::<Vec<String>>();
            db.tool_vector_search_with_vector_limited(embedding, 5, tool_names)
        } else {
            db.tool_vector_search(&sanitized_query, 5, false, true).await
        };

        let vector_elapsed_time = vector_start_time.elapsed();
        println!("Time taken for vector search: {:?}", vector_elapsed_time);

        let fts_start_time = std::time::Instant::now();
        let fts_search_result = db.search_tools_fts(&sanitized_query);
        let fts_elapsed_time = fts_start_time.elapsed();
        println!("Time taken for FTS search: {:?}", fts_elapsed_time);

        match (vector_search_result, fts_search_result) {
            (Ok(vector_tools), Ok(fts_tools)) => {
                let mut combined_tools = Vec::new();
                let mut seen_ids = std::collections::HashSet::new();

                if let Some(first_fts_tool) = fts_tools.first() {
                    if seen_ids.insert(first_fts_tool.tool_router_key.clone()) {
                        combined_tools.push(first_fts_tool.clone());
                    }
                }

                if let Some((tool, score)) = vector_tools.first() {
                    if *score < 0.2 {
                        if seen_ids.insert(tool.tool_router_key.clone()) {
                            combined_tools.push(tool.clone());
                        }
                    }
                }

                for tool in fts_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                for (tool, _) in vector_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                let tools_json = serde_json::to_value(combined_tools).map_err(|err| NodeError {
                    message: format!("Failed to serialize tools: {}", err),
                })?;

                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    let result_count = tools_json.as_array().map_or(0, |arr| arr.len());
                    println!("Time taken for tool search: {:?}", elapsed_time);
                    println!("Number of tool results: {}", result_count);
                }
                let _ = res.send(Ok(tools_json)).await;
                Ok(())
            }
            (Err(err), _) | (_, Err(err)) => {
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
        db: Arc<SqliteManager>,
        bearer: String,
        node_name: ShinkaiName,
        category: Option<String>,
        tool_router: Option<Arc<crate::managers::tool_router::ToolRouter>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match db.get_all_tool_headers() {
            Ok(tools) => {
                use std::collections::HashMap;
                let mut tool_groups: HashMap<String, Vec<shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader>> = HashMap::new();

                for tool in tools {
                    let tool_router_key = tool.tool_router_key.clone();
                    tool_groups.entry(tool_router_key).or_default().push(tool);
                }

                let mut latest_tools = Vec::new();
                for (_, mut group) in tool_groups {
                    if group.len() == 1 {
                        latest_tools.push(group.pop().unwrap());
                    } else {
                        group.sort_by(|a, b| {
                            let a_version = IndexableVersion::from_string(&a.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            let b_version = IndexableVersion::from_string(&b.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            b_version.cmp(&a_version)
                        });

                        latest_tools.push(group.remove(0));
                    }
                }

                let filtered_tools = if let Some(category) = category {
                    match category.to_lowercase().as_str() {
                        "downloaded" => {
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            let node_name_string = node_name.get_node_name_string();

                            latest_tools
                                .into_iter()
                                .filter(|tool| {
                                    let is_not_default = if let Some(default_keys) = &default_tool_keys {
                                        !default_keys.contains(&tool.tool_router_key)
                                    } else {
                                        true // If we can't determine default tools, assume it's not default
                                    };

                                    let is_not_localhost = !tool.author.starts_with("localhost.");

                                    let is_not_node_name = tool.author != node_name_string;

                                    let is_not_rust = !matches!(tool.tool_type.to_lowercase().as_str(), "rust");

                                    is_not_default && is_not_localhost && is_not_node_name && is_not_rust
                                })
                                .collect()
                        }
                        "default" => {
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            if let Some(default_keys) = &default_tool_keys {
                                latest_tools
                                    .into_iter()
                                    .filter(|tool| default_keys.contains(&tool.tool_router_key))
                                    .collect()
                            } else {
                                latest_tools
                            }
                        }
                        "system" => latest_tools
                            .into_iter()
                            .filter(|tool| matches!(tool.tool_type.to_lowercase().as_str(), "rust"))
                            .collect(),
                        "my_tools" => {
                            let node_name_string = node_name.get_node_name_string();
                            latest_tools
                                .into_iter()
                                .filter(|tool| tool.author.starts_with("localhost.") || tool.author == node_name_string)
                                .collect()
                        }
                        _ => latest_tools, // If an unknown category is provided, return all tools
                    }
                } else {
                    latest_tools
                };

                let t = filtered_tools.iter().map(|tool| json!(tool)).collect();
                let _ = res.send(Ok(t)).await;
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
        db: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        input_value: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let existing_tool = match db.get_tool_by_key(&tool_router_key) {
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

        let merged_value = Self::merge_json(existing_tool_value, input_value);

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

        let save_result = db.update_tool(merged_tool).await;

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
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        new_tool_with_assets: ShinkaiToolWithAssets,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let new_tool = new_tool_with_assets.tool;
        let dependencies = new_tool.get_tools();
        for dependency in dependencies {
            let tool = db.get_tool_by_key(&dependency.to_string_without_version());
            if tool.is_err() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Tool not found: {}", dependency.to_string_without_version()),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        let save_result = db.add_tool(new_tool).await;

        match save_result {
            Ok(tool) => {
                let tool_key = tool.tool_router_key();

                if let Some(assets) = new_tool_with_assets.assets {
                    if !assets.is_empty() {
                        let file_path = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
                            .join(".tools_storage")
                            .join("tools")
                            .join(tool.tool_router_key().convert_to_path());
                        if !file_path.exists() {
                            let s = std::fs::create_dir_all(&file_path);
                            if s.is_err() {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Failed to create directory".to_string(),
                                    message: format!("Failed to create directory: {}", s.err().unwrap()),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        for asset in assets {
                            let asset_path = file_path.join(asset.file_name);
                            let asset_content = base64::decode(asset.data).unwrap();
                            let status = fs::write(asset_path, asset_content).await;
                            if status.is_err() {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Failed to create directory".to_string(),
                                    message: format!("Failed to create directory: {}", status.err().unwrap()),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                    }
                }

                let response = json!({ "status": "success", "message": format!("Tool added with key: {}", tool_key.to_string_without_version()) });
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
        db: Arc<SqliteManager>,
        bearer: String,
        payload: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match db.get_tool_by_key(&payload) {
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
}
