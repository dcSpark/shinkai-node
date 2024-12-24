use shinkai_message_primitives::schemas::job::JobLike;
use shinkai_message_primitives::schemas::subprompts::SubPrompt;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;

use tokio::sync::Mutex;

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
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("map_function".to_string(), "string".to_string(), "The map function to use".to_string(), false);
                    params.add_property("prompt".to_string(), "string".to_string(), "The prompt to use".to_string(), true);
                    params
                },
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
        db_clone: Arc<SqliteManager>,
        node_name: ShinkaiName,
        _identity_manager_clone: Arc<Mutex<IdentityManager>>,
        _job_manager: Arc<Mutex<JobManager>>,
        _encryption_secret_key_clone: EncryptionStaticKey,
        _encryption_public_key_clone: EncryptionPublicKey,
        _signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        _llm_provider: String,
    ) -> Result<Value, ToolError> {
        // TODO: define parameters (job: check and direct inbox_id: missing)
        // TODO: how do we use app_id here? is it linked to a job somehow?
        // TODO: create e2e test using this fn so we can test it with some real data

        let mut scope = MinimalJobScope::default();

        // Checks if job_id is provided in the parameters
        if let Some(job_id_value) = parameters.get("job_id") {
            if let Some(job_id) = job_id_value.as_str() {
                // Fetch the job data using the correct method
                let fetch_data_result = JobManager::fetch_relevant_job_data(job_id, db_clone.clone()).await;
                let (full_job, _llm_provider_found, _, _user_profile) = match fetch_data_result {
                    Ok(data) => data,
                    Err(e) => return Err(ToolError::ExecutionError(format!("Failed to fetch job data: {}", e))),
                };

                scope = full_job.scope().clone();
            }
        }

        // Use the new method to retrieve resources
        let resource_collections = JobManager::retrieve_all_resources_in_job_scope(&scope, &db_clone)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to retrieve resources: {:?}", e)))?;

        let mut processed_embeddings = Vec::new();

        for collection in resource_collections {
            let subprompts = SubPrompt::convert_chunks_into_subprompts_with_extra_info(&collection.chunks, 97);
            let embedding = subprompts
                .iter()
                .map(|subprompt| subprompt.get_content().clone())
                .collect::<Vec<String>>()
                .join(" ");
            processed_embeddings.push(embedding);
        }

        let joined_results = processed_embeddings.join(":::");
        Ok(json!({
            "result": joined_results,
            "type": "embeddings",
            "rowCount": processed_embeddings.len(),
            "rowsAffected": processed_embeddings.len(),
        }))
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
