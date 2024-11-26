use futures::StreamExt;
use shinkai_message_primitives::schemas::job::JobLike;
use shinkai_message_primitives::schemas::subprompts::SubPrompt;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::argument::ToolArgument;
use shinkai_tools_primitives::tools::{argument::ToolOutputArg, shinkai_tool::ShinkaiToolHeader};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use std::sync::{Arc, Weak};

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;

use tokio::sync::{Mutex, RwLock};

use async_trait::async_trait;

use crate::tools::tool_implementation::tool_traits::ToolExecutor;

// LLM Tool
pub struct KnowledgeTool {
    pub tool: ShinkaiToolHeader,
}

impl KnowledgeTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Process Embeddings".to_string(),
                toolkit_name: "shinkai_custom".to_string(),
                description: r#"Tool for processing embeddings within a job scope. 
                This tool processes resources and generates embeddings using a specified mapping function.
                
                Example usage:
                - Provide a custom mapping function to transform resource content.
                - Process resources in chunks to optimize performance.
                - Collect and join processed embeddings for further analysis."#
                    .to_string(),
                tool_router_key: "local:::rust_toolkit:::shinkai_process_embeddings".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Process embeddings in job scope".to_string(),
                author: "Shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                input_args: vec![
                    ToolArgument::new(
                        "map_function".to_string(),
                        "function".to_string(),
                        "A function to map over resource content".to_string(),
                        false,
                    ),
                ],
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"result": {"type": "string"}, "type": {"type": "string"}, "rowCount": {"type": "number"}, "rowsAffected": {"type": "number"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            }
        }
    }
}

#[async_trait]
impl ToolExecutor for KnowledgeTool {
    async fn execute(
        _bearer: String,
        _tool_id: String,
        _app_id: String,
        db_clone: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        _sqlite_manager: Arc<RwLock<SqliteManager>>,
        node_name: ShinkaiName,
        _identity_manager_clone: Arc<Mutex<IdentityManager>>,
        _job_manager: Arc<Mutex<JobManager>>,
        _encryption_secret_key_clone: EncryptionStaticKey,
        _encryption_public_key_clone: EncryptionPublicKey,
        _signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        _llm_provider: String,
    ) -> Result<Value, ToolError> {
        // TODO: define parameters
        // TODO: how do we use app_id here? is it linked to a job somehow?
        // TODO: create e2e test using this fn so we can test it with some real data

        let mut scope = JobScope::new_default();

        // Checks if job_id is provided in the parameters
        if let Some(job_id_value) = parameters.get("job_id") {
            if let Some(job_id) = job_id_value.as_str() {
                // Fetch the job data using the correct method
                let fetch_data_result = JobManager::fetch_relevant_job_data(job_id, db_clone.clone()).await;
                let (full_job, _llm_provider_found, _, _user_profile) = match fetch_data_result {
                    Ok(data) => data,
                    Err(e) => return Err(ToolError::ExecutionError(format!("Failed to fetch job data: {}", e))),
                };

                if let Some(scope_with_files) = full_job.scope_with_files().clone() {
                    scope = scope_with_files.clone();
                } else {
                    return Err(ToolError::ExecutionError(
                        "Failed to extract scope with files".to_string(),
                    ));
                }
            }
        }

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Runtime::new()
                .map_err(|e| ToolError::ExecutionError(e.to_string()))?
                .block_on(async {
                    // TODO: if scope empty then return an error?

                    let resource_stream = {
                        // Note(Nico): in the future we will get rid of this old fashion way to do embeddings
                        let user_profile =
                            ShinkaiName::from_node_and_profile_names(node_name.node_name, "main".to_string()).unwrap();

                        JobManager::retrieve_all_resources_in_job_scope_stream(vector_fs.clone(), &scope, &user_profile)
                            .await
                    };

                    let mut chunks = resource_stream.chunks(5);
                    let mut processed_embeddings = Vec::new();

                    while let Some(resources) = chunks.next().await {
                        let futures = resources.into_iter().map(|resource| async move {
                            let subprompts = SubPrompt::convert_resource_into_subprompts_with_extra_info(&resource, 97);
                            let embedding = subprompts
                                .iter()
                                .map(|subprompt| subprompt.get_content().clone())
                                .collect::<Vec<String>>()
                                .join(" ");
                            Ok::<_, ToolError>(embedding)
                        });

                        let results = futures::future::join_all(futures).await;

                        for result in results {
                            match result {
                                Ok(processed) => processed_embeddings.push(processed),
                                Err(e) => {
                                    // Log error but continue processing
                                    eprintln!("Error processing embedding: {}", e);
                                }
                            }
                        }
                    }

                    let joined_results = processed_embeddings.join(":::");
                    Ok::<_, ToolError>(json!({
                        "result": joined_results,
                        "type": "embeddings",
                        "rowCount": processed_embeddings.len(),
                        "rowsAffected": processed_embeddings.len(),
                    }))
                })
        })?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_router_key() {
        let knowledge_tool = KnowledgeTool::new();
        assert_eq!(
            knowledge_tool.tool.tool_router_key,
            "local:::rust_toolkit:::shinkai_process_embeddings"
        );
    }
}
