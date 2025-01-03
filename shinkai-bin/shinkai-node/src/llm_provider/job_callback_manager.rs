use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{cron_tasks::cron_manager::CronManager, managers::sheet_manager::SheetManager};

use super::job_manager::JobManager;
use crate::llm_provider::error::LLMProviderError;
use crate::network::Node;
use crate::tools::tool_execution::execution_coordinator::check_code;
use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_sqlite::SqliteManager;
use tokio::sync::RwLock;

/// The `JobCallbackManager` is responsible for handling incoming job requests
/// and delegating them to the appropriate manager (JobManager, SheetManager, or CronManager).
///
/// # Fields
/// - `job_manager`: An `Arc<Mutex<JobManager>>` for handling job-related requests.
/// - `sheet_manager`: An `Arc<Mutex<SheetManager>>` for handling sheet-related requests.
/// - `cron_manager`: An `Arc<Mutex<CronManager>>` for handling cron-related requests.
///
/// # Methods
/// - `new`: Creates a new instance of `JobCallbackManager` with the provided managers.
/// - `handle_request`: Takes a `JobRequest` and forwards it to the appropriate manager based on the `manager_type`.
pub struct JobCallbackManager {
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    pub sheet_manager: Option<Arc<Mutex<SheetManager>>>,
    pub cron_manager: Option<Arc<Mutex<CronManager>>>,
}

// TODO: allow for chaining of multiple jobs some of the jobs may give a result that's used by another job A -> B -> C
impl Default for JobCallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobCallbackManager {
    pub fn new() -> Self {
        JobCallbackManager {
            job_manager: None,
            sheet_manager: None,
            cron_manager: None,
        }
    }

    pub fn update_job_manager(&mut self, job_manager: Arc<Mutex<JobManager>>) {
        self.job_manager = Some(job_manager);
    }

    pub fn update_sheet_manager(&mut self, sheet_manager: Arc<Mutex<SheetManager>>) {
        self.sheet_manager = Some(sheet_manager);
    }

    pub fn update_cron_manager(&mut self, cron_manager: Arc<Mutex<CronManager>>) {
        self.cron_manager = Some(cron_manager);
    }

    // pub async fn handle_request(&self, action: CallbackAction) {
    //     match action {
    //         // CallbackAction::Job(job_message) => {
    //         //     let mut manager = self.job_manager.lock().await;
    //         //     // manager.handle_job(job_message);
    //         // }
    //         CallbackAction::Sheet(sheet_action) => {
    //             let mut manager = self.sheet_manager.lock().await;
    //             // manager.handle_sheet(sheet_action);
    //         }
    //         // Note: add later
    //         // CallbackAction::Cron(cron_action) => {
    //         //     let mut manager = self.cron_manager.lock().await;
    //         //     // manager.handle_cron(cron_action);
    //         // }
    //     }

    //     // // Handle nested callbacks
    //     // if let Some(callback) = action.get_callback() {
    //     //     self.handle_request(*callback);
    //     // }
    // }

    pub async fn handle_implementation_check_callback(
        &self,
        db: Arc<SqliteManager>,
        tool_type: DynamicToolType,
        inference_response_content: String,
        available_tools: Vec<ToolRouterKey>,
        identity_secret_key: &SigningKey,
        user_profile: &ShinkaiName,
        job_id: &str,
    ) -> Result<(), LLMProviderError> {
        let result = check_code(
            tool_type.clone(),
            inference_response_content.clone(),
            "".to_string(),
            "".to_string(),
            available_tools.clone(),
            db.clone(),
        )
        .await?;

        // Return early if result is empty
        if result.is_empty() {
            return Ok(());
        }

        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let error_message = format!("Code implementation check failed: {:?}", result);

        // Create a ShinkaiMessage that looks it came from the user
        let shinkai_message = ShinkaiMessageBuilder::job_message_unencrypted(
            job_id.to_string(),
            error_message,
            vec![],
            "".to_string(),
            identity_secret_key_clone,
            user_profile.node_name.clone(),
            user_profile.get_profile_name_string().unwrap_or("main".to_string()),
            user_profile.node_name.clone(),
            "".to_string(),
        )
        .map_err(|e| LLMProviderError::ShinkaiMessageBuilderError(e.to_string()))?;

        let job_manager = {
            let callback_manager = self;
            callback_manager.job_manager.clone()
        };

        if let Some(job_manager) = job_manager {
            let _result = Node::internal_job_message(job_manager, shinkai_message.clone()).await;
        } else {
            eprintln!("Job manager is not set in JobCallbackManager");
        }

        Ok(())
    }
}
