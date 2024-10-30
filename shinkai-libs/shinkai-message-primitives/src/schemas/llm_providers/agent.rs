use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::schemas::{job_config::JobConfig, shinkai_name::ShinkaiName};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Agent {
    pub name: String,
    pub agent_id: String,
    pub full_identity_name: ShinkaiName,
    pub llm_provider_id: String,
    pub instructions: String,
    pub ui_description: String,
    pub knowledge: Vec<String>,
    pub storage_path: String,
    pub tools: Vec<String>,
    pub debug_mode: bool,
    pub config: Option<JobConfig>,
}
