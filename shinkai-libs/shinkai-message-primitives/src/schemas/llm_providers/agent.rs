use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::schemas::{job_config::JobConfig, shinkai_name::ShinkaiName, tool_router_key::ToolRouterKey};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Agent {
    pub name: String,
    pub agent_id: String,
    pub full_identity_name: ShinkaiName,
    pub llm_provider_id: String, // Connected
    // pub instructions: String, // TODO: maybe we can remove on post to custom_prompt -- not super clean but not repetitive
    pub ui_description: String,
    pub knowledge: Vec<String>,    // TODO
    pub storage_path: String,      // TODO
    pub tools: Vec<ToolRouterKey>, // Connected
    pub debug_mode: bool,          // TODO
    pub config: Option<JobConfig>, // Connected
}
