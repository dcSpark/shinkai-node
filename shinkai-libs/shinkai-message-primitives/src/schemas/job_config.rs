use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct JobConfig {
    pub custom_system_prompt: Option<String>,
    pub custom_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub seed: Option<u64>,
    pub top_k: Option<u64>,
    pub top_p: Option<f64>,
    pub stream: Option<bool>,
    pub other_model_params: Option<Value>,
    pub use_tools: Option<bool>,
    // TODO: add ctx_...
}

impl JobConfig {
    /// Merges two JobConfig instances, preferring values from `self` over `other`.
    pub fn merge(&self, other: &JobConfig) -> JobConfig {
        JobConfig {
            // Prefer `self` (provided config) over `other` (agent's config)
            custom_system_prompt: self.custom_system_prompt.clone().or_else(|| other.custom_system_prompt.clone()),
            custom_prompt: self.custom_prompt.clone().or_else(|| other.custom_prompt.clone()),
            temperature: self.temperature.or(other.temperature),
            max_tokens: self.max_tokens.or(other.max_tokens),
            seed: self.seed.or(other.seed),
            top_k: self.top_k.or(other.top_k),
            top_p: self.top_p.or(other.top_p),
            stream: self.stream.or(other.stream),
            use_tools: self.use_tools.or(other.use_tools),
            other_model_params: self
                .other_model_params
                .clone()
                .or_else(|| other.other_model_params.clone()),
        }
    }

    /// Creates an empty JobConfig with all fields set to None.
    pub fn empty() -> JobConfig {
        JobConfig {
            custom_system_prompt: None,
            custom_prompt: None,
            temperature: None,
            max_tokens: None,
            seed: None,
            top_k: None,
            top_p: None,
            stream: None,
            other_model_params: None,
            use_tools: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_job_config() {
        let json_data = r#"{
            "custom_system_prompt": null,
            "custom_prompt": "",
            "temperature": 0.8,
            "max_tokens": null,
            "seed": null,
            "top_k": 40,
            "top_p": 0.9,
            "stream": true,
            "other_model_params": null,
            "use_tools": false
        }"#;

        let job_config: JobConfig = serde_json::from_str(json_data).expect("Failed to deserialize JSON");

        assert_eq!(job_config.custom_system_prompt, None);
        assert_eq!(job_config.custom_prompt, Some("".to_string()));
        assert_eq!(job_config.temperature, Some(0.8));
        assert_eq!(job_config.max_tokens, None);
        assert_eq!(job_config.seed, None);
        assert_eq!(job_config.top_k, Some(40));
        assert_eq!(job_config.top_p, Some(0.9));
        assert_eq!(job_config.stream, Some(true));
        assert_eq!(job_config.other_model_params, None);
        assert_eq!(job_config.use_tools, Some(false));
    }
}
