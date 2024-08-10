use std::sync::Arc;
use std::time::Instant;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APISetWorkflow;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIWorkflowKeyname;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB,
    managers::IdentityManager,
    network::{node_api_router::APIError, node_error::NodeError, Node},
    schemas::identity::Identity,
    tools::{shinkai_tool::ShinkaiTool, tool_router::ToolRouter, workflow_tool::WorkflowTool},
};

use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn v2_api_search_workflows(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
        bearer: String,
        query: String,
        embedding_generator: Arc<RemoteEmbeddingGenerator>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
    
        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Start the timer
        let start_time = Instant::now();
    
        // Generate the embedding for the search query
        let embedding = match embedding_generator.generate_embedding_default(&query).await {
            Ok(embedding) => embedding,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate embedding: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
    
        // Perform the internal search using tool_router
        if let Some(tool_router) = tool_router {
            let mut tool_router = tool_router.lock().await;
            match tool_router
                .workflow_search(
                    requester_name,
                    Box::new((*embedding_generator).clone()) as Box<dyn EmbeddingGenerator>,
                    db,
                    embedding,
                    &query,
                    5,
                )
                .await
            {
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
        } else {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Tool router is not available".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            Ok(())
        }
    }
    
    pub async fn v2_api_set_workflow(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
        generator: Arc<RemoteEmbeddingGenerator>,
        bearer: String,
        payload: APISetWorkflow,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

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

        // Save the workflow to the database
        match db.save_workflow(workflow.clone(), requester_name.clone()) {
            Ok(_) => {
                // Add the workflow to the tool_router
                if let Some(tool_router) = tool_router {
                    let mut tool_router = tool_router.lock().await;

                    // Create a WorkflowTool from the workflow
                    let mut workflow_tool = WorkflowTool::new(workflow.clone());

                    // Generate an embedding for the workflow
                    let embedding_text = workflow_tool.format_embedding_string();
                    let embedding = match generator.generate_embedding_default(&embedding_text).await {
                        Ok(emb) => emb,
                        Err(err) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to generate embedding: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
                    };

                    // Set the embedding in the WorkflowTool
                    workflow_tool.embedding = Some(embedding.clone());

                    // Create a ShinkaiTool::Workflow
                    let shinkai_tool = ShinkaiTool::Workflow(workflow_tool);

                    // Add the tool to the ToolRouter
                    match tool_router.add_shinkai_tool(&requester_name, &shinkai_tool, embedding) {
                        Ok(_) => {
                            let response =
                                json!({ "status": "success", "message": "Workflow added to database and tool router" });
                            let _ = res.send(Ok(response)).await;
                        }
                        Err(err) => {
                            // If adding to tool_router fails, we should probably remove the workflow from the database
                            db.remove_workflow(&workflow.generate_key(), &requester_name)?;
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to add workflow to tool router: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                        }
                    }
                } else {
                    let response = json!({ "status": "partial_success", "message": "Workflow added to database, but tool router is not available" });
                    let _ = res.send(Ok(response)).await;
                }
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to save workflow: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_remove_workflow(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Generate the workflow key
        let workflow_key_str = payload.generate_key();

        // Remove the workflow from the database
        match db.remove_workflow(&workflow_key_str, &requester_name) {
            Ok(_) => {
                // Remove the workflow from the tool_router
                if let Some(tool_router) = tool_router {
                    let mut tool_router = tool_router.lock().await;
                    match tool_router.delete_shinkai_tool(&requester_name, &payload.name, "workflow") {
                        Ok(_) => {
                            let response = json!({ "status": "success", "message": "Workflow removed from database and tool router" });
                            let _ = res.send(Ok(response)).await;
                        }
                        Err(err) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to remove workflow from tool router: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                        }
                    }
                } else {
                    let response = json!({ "status": "partial_success", "message": "Workflow removed from database, but tool router is not available" });
                    let _ = res.send(Ok(response)).await;
                }
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove workflow: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_workflow_info(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Generate the workflow key
        let workflow_key_str = payload.generate_key();

        // Get the workflow from the database
        match db.get_workflow(&workflow_key_str, &requester_name) {
            Ok(workflow) => {
                let response = json!(workflow);
                let _ = res.send(Ok(response)).await;
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
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
        generator: Arc<RemoteEmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Check if tool_router is available and has started if not start it
        if let Some(tool_router) = &tool_router {
            let mut tool_router = tool_router.lock().await;
            if !tool_router.is_started() {
                if let Err(err) = tool_router
                    .start(
                        Box::new((*generator).clone()),
                        Arc::downgrade(&db),
                        requester_name.clone(),
                    )
                    .await
                {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to start tool router: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        // List all workflows for the user
        match db.list_all_workflows_for_user(&requester_name) {
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
}
