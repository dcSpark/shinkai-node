use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct JobConfig {
    pub custom_prompt: Option<String>,
    // pub custom_system_prompt: String
    pub temperature: Option<f64>,
    // pub max_output_tokens: u64,
    pub seed: Option<u64>,
    pub top_k: Option<u64>,
    pub top_p: Option<f64>,
    pub stream: Option<bool>,
    pub other_model_params: Option<Value>,
}