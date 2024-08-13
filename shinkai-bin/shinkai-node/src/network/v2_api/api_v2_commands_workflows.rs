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
}
