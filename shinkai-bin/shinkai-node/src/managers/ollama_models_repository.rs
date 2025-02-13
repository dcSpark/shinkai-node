use serde::{Deserialize, Serialize};

lazy_static::lazy_static! {
    // Explain this file is currently generated in the shinkai desktop repository
    // and should be kept in sync with the desktop repository
    static ref OLLAMA_MODELS_REPOSITORY_JSON: &'static str = include_str!("./ollama-models-repository.json");
    static ref OLLAMA_MODELS_REPOSITORY: std::collections::HashMap<String, OllamaModel> = {
        let mut map = std::collections::HashMap::new();
        let models: Vec<OllamaModel> = serde_json::from_str(&OLLAMA_MODELS_REPOSITORY_JSON).unwrap();
        for model in models {
            map.insert(model.name.clone(), model);
        }
        map
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModelTag {
    pub name: String,
    pub hash: String,
    pub size: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub description: String,
    #[serde(rename = "supportTools")]
    pub support_tools: bool,
    #[serde(rename = "defaultTag")]
    pub default_tag: String,
    pub tags: Vec<OllamaModelTag>,
}

pub struct OllamaModelsRepository;

impl OllamaModelsRepository {
    pub fn get_base_model_name(model_name: &str) -> String {
        match model_name.split(':').next() {
            Some(base_name) => base_name.to_string(),
            None => model_name.to_string(),
        }
    }
    pub fn get_models() -> Vec<&'static OllamaModel> {
        OLLAMA_MODELS_REPOSITORY.values().collect()
    }
    pub fn get_model_by_name(model_name: &str) -> Option<&'static OllamaModel> {
        let base_name = Self::get_base_model_name(model_name);
        let model = OLLAMA_MODELS_REPOSITORY.get(&base_name);
        if model.is_none() {
            log::warn!(
                "Ollama model {} not found (the repository could be outdated)",
                base_name
            );
        }
        model
    }
    pub fn supports_tools(model_name: &str) -> bool {
        let model = Self::get_model_by_name(&model_name);
        if let Some(model) = model {
            model.support_tools
        } else {
            log::warn!(
                "Ollama model {} not found (the repository could be outdated), assuming it does not support tools",
                model_name
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_models() {
        let models = OllamaModelsRepository::get_models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_get_model_by_name() {
        let model = OllamaModelsRepository::get_model_by_name("llama3.1:8b");
        assert!(model.is_some());
    }

    #[test]
    fn test_get_model_by_name_no_tag() {
        let model = OllamaModelsRepository::get_model_by_name("llama3.1");
        assert!(model.is_some());
    }

    #[test]
    fn test_supports_tools() {
        let supports_tools = OllamaModelsRepository::supports_tools("llama3.1:8b");
        assert!(supports_tools);
    }

    #[test]
    fn test_all_tool_supporting_models() {
        // Test llama3.1/3.2 variants
        assert!(OllamaModelsRepository::supports_tools("llama3.1:8b"));
        assert!(OllamaModelsRepository::supports_tools("llama3.2:70b"));

        // Test mistral variants
        assert!(OllamaModelsRepository::supports_tools("mistral-nemo:7b"));
        assert!(OllamaModelsRepository::supports_tools("mistral-small:7b"));
        assert!(OllamaModelsRepository::supports_tools("mistral-large:7b"));

        // Test qwen variants
        assert!(OllamaModelsRepository::supports_tools("qwen2.5-coder:7b"));
        assert!(OllamaModelsRepository::supports_tools("qwq:7b"));

        // Test command variant
        assert!(OllamaModelsRepository::supports_tools("command-r7b"));

        // Test mistral-small again (duplicate in original list)
        assert!(OllamaModelsRepository::supports_tools("mistral-small:7b"));
    }

    #[test]
    fn test_qwen2_supports_tools() {
        assert!(OllamaModelsRepository::supports_tools("qwen2:7b"));
    }
}
