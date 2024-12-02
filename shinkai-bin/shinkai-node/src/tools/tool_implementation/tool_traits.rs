use ed25519_dalek::SigningKey;
use serde_json::{Map, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, RwLock};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{llm_provider::job_manager::JobManager, managers::IdentityManager};

#[async_trait::async_trait]
pub trait ToolExecutor {
    async fn execute(
        bearer: String,
        tool_id: String,
        app_id: String,
        db_clone: Arc<ShinkaiDB>,
        vector_fs_clone: Arc<VectorFS>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        llm_provider: String,
    ) -> Result<Value, ToolError>;
}
