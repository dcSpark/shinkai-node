use crate::{
    llm_provider::job_manager::JobManager,
    managers::{tool_router::ToolRouter, IdentityManager},
    network::{node_error::NodeError, Node},
    tools::{
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_deno_tools},
        tool_execution::execution_coordinator::{execute_code, execute_tool_cmd},
    },
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    shinkai_tools::{CodeLanguage, DynamicToolType},
    tool_router_key::ToolRouterKey,
};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    tool_config::{OAuth, ToolConfig},
    tool_types::{OperatingSystem, RunnerType},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn get_tool_definitions(
        bearer: String,
        db: Arc<SqliteManager>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let definitions = generate_tool_definitions(tools, language, db, false).await;
        match definitions {
            Ok(definitions) => {
                let mut map: Map<String, Value> = Map::new();
                definitions.into_iter().for_each(|(key, value)| {
                    map.insert(key, Value::String(value));
                });

                let _ = res.send(Ok(Value::Object(map))).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }
        Ok(())
    }

    pub async fn execute_tool(
        bearer: String,
        node_name: ShinkaiName,
        db: Arc<SqliteManager>,
        tool_router_key: String,
        parameters: Map<String, Value>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        extra_config: Map<String, Value>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        mounts: Option<Vec<String>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_configs = ToolConfig::basic_config_from_value(&Value::Object(extra_config));

        let result = execute_tool_cmd(
            bearer,
            node_name,
            db,
            tool_router_key.clone(),
            parameters,
            tool_configs,
            None,
            tool_id,
            app_id,
            llm_provider,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
            mounts,
        )
        .await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_router_key);
                let _ = res.send(Ok(result)).await;
            }
            Err(e) => {
                println!("[execute_command] Tool execution failed {}: {}", tool_router_key, e);
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error executing tool: {}", e),
                    }))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn run_execute_code(
        bearer: String,
        db: Arc<SqliteManager>,
        tool_type: DynamicToolType,
        code: String,
        tools: Vec<ToolRouterKey>,
        parameters: Map<String, Value>,
        extra_config: Map<String, Value>,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        node_name: ShinkaiName,
        mounts: Option<Vec<String>>,
        runner: Option<RunnerType>,
        operating_system: Option<Vec<OperatingSystem>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_configs = ToolConfig::basic_config_from_value(&Value::Object(extra_config));


        let result = execute_code(
            tool_type.clone(),
            code,
            tools,
            parameters,
            tool_configs,
            oauth,
            db,
            tool_id,
            app_id,
            llm_provider,
            bearer,
            node_name,
            mounts,
            runner,
            operating_system,
        )
        .await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_type);
                let _ = res.send(Ok(result)).await;
            }
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error executing tool: {}", e),
                    }))
                    .await;
            }
        }

        Ok(())
    }
}
