use serde::{Deserialize, Serialize};

use super::job_config::JobConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub agent_id: String,
    pub llm_provider_id: String,
    pub instructions: String,
    pub ui_description: String,
    pub knowledge: Vec<String>,
    pub storage_path: String,
    pub tools: Vec<String>,
    pub debug_mode: bool,
    pub config: Option<JobConfig>,
}
