use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
    tools::{execute_tool, generate_tool_definitions},
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{Map, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    shinkai_tools::{Language, ToolType},
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;
impl Node {
    pub async fn generate_tool_definitions(
        bearer: String,
        db: Arc<ShinkaiDB>,
        language: Language,
        sqlite_manager: Arc<SqliteManager>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let definitions = generate_tool_definitions(language, sqlite_manager, false).await;

        match definitions {
            Ok(definitions) => {
                let _ = res.send(Ok(Value::String(definitions))).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }
        Ok(())
    }

    pub async fn execute_command(
        bearer: String,
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        tool_router_key: String,
        tool_type: ToolType,
        parameters: Map<String, Value>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Execute the tool directly
        let result = execute_tool(
            tool_router_key.clone(),
            tool_type,
            parameters,
            None,
            db,
            sqlite_manager,
            bearer,
            node_name,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
        )
        .await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_router_key);
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

    pub async fn generate_tool_implementation(
        bearer: String,
        language: Language,
        code: Option<String>,
        metadata: Option<String>,
        output: Option<String>,
        prompt: String,
        sqlite_manager: Arc<SqliteManager>,
        db_clone: Arc<ShinkaiDB>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        job_creation_info: JobCreationInfo,
        llm_provider: String,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        raw: bool,
        fetch_query: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db_clone.clone(), &res)
            .await
            .is_err()
        {
            return Ok(());
        }
        // Generate the implementation
        let implementation = crate::tools::tool_generation::tool_implementation(
            bearer,
            language,
            code,
            metadata,
            output,
            Some(prompt),
            sqlite_manager,
            db_clone,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            job_creation_info,
            llm_provider,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            raw,
            fetch_query,
        )
        .await;

        match implementation {
            Ok(implementation_) => {
                let _ = res.send(Ok(implementation_)).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }

        Ok(())
    }

    pub async fn generate_tool_metadata_implementation(
        bearer: String,
        language: Language,
        code: Option<String>,
        metadata: Option<String>,
        output: Option<String>,
        sqlite_manager: Arc<SqliteManager>,
        db_clone: Arc<ShinkaiDB>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        job_creation_info: JobCreationInfo,
        llm_provider: String,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,

        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db_clone.clone(), &res)
            .await
            .is_err()
        {
            return Ok(());
        }
        // Generate the implementation
        let metadata = crate::tools::tool_generation::tool_metadata_implementation(
            bearer,
            language,
            code,
            metadata,
            output,
            sqlite_manager,
            db_clone,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            job_creation_info,
            llm_provider,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await;

        match metadata {
            Ok(metadata_) => {
                let _ = res.send(Ok(metadata_)).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }

        Ok(())
    }
}
