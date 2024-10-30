use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct JobConfig {
    pub custom_prompt: Option<String>,
    // pub custom_system_prompt: String
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub seed: Option<u64>,
    pub top_k: Option<u64>,
    pub top_p: Option<f64>,
    pub stream: Option<bool>,
    pub other_model_params: Option<Value>,
    // TODO: add ctx_...
}

impl JobConfig {
    /// Merges two JobConfig instances, preferring values from `self` over `other`.
    pub fn merge(&self, other: &JobConfig) -> JobConfig {
        JobConfig {
            // Prefer `self` (provided config) over `other` (agent's config)
            custom_prompt: self.custom_prompt.clone().or_else(|| other.custom_prompt.clone()),
            temperature: self.temperature.or(other.temperature),
            max_tokens: self.max_tokens.or(other.max_tokens),
            seed: self.seed.or(other.seed),
            top_k: self.top_k.or(other.top_k),
            top_p: self.top_p.or(other.top_p),
            stream: self.stream.or(other.stream),
            other_model_params: self
                .other_model_params
                .clone()
                .or_else(|| other.other_model_params.clone()),
        }
    }

    /// Creates an empty JobConfig with all fields set to None.
    pub fn empty() -> JobConfig {
        JobConfig {
            custom_prompt: None,
            temperature: None,
            max_tokens: None,
            seed: None,
            top_k: None,
            top_p: None,
            stream: None,
            other_model_params: None,
        }
    }
}
