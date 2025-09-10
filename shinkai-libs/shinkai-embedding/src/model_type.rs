use std::fmt;
use std::hash::Hash;

use crate::shinkai_embedding_errors::ShinkaiEmbeddingError;

pub type EmbeddingModelTypeString = String;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum EmbeddingModelType {
    OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference),
}

impl EmbeddingModelType {
    pub fn from_string(s: &str) -> Result<Self, ShinkaiEmbeddingError> {
        OllamaTextEmbeddingsInference::from_string(s)
            .map(EmbeddingModelType::OllamaTextEmbeddingsInference)
            .map_err(|_| ShinkaiEmbeddingError::InvalidModelArchitecture)
    }

    /// Returns the default embedding model
    pub fn default() -> Self {
        std::env::var("DEFAULT_EMBEDDING_MODEL")
            .and_then(|s| Self::from_string(&s).map_err(|_| std::env::VarError::NotPresent))
            .unwrap_or_else(|_| {
                EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::EmbeddingGemma300M)
            })
    }

    pub fn max_input_token_count(&self) -> usize {
        match self {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => model.max_input_token_count(),
        }
    }

    pub fn embedding_normalization_factor(&self) -> f32 {
        match self {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => model.embedding_normalization_factor(),
        }
    }

    pub fn vector_dimensions(&self) -> Result<usize, ShinkaiEmbeddingError> {
        match self {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => model.vector_dimensions(),
        }
    }
}

impl fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => write!(f, "{}", model),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OllamaTextEmbeddingsInference {
    AllMiniLML6v2,
    #[serde(alias = "SnowflakeArcticEmbed_M")]
    SnowflakeArcticEmbedM,
    JinaEmbeddingsV2BaseEs,
    EmbeddingGemma300M,
    Other(String),
}

impl OllamaTextEmbeddingsInference {
    const ALL_MINI_LML6V2: &'static str = "all-minilm:l6-v2";
    const SNOWFLAKE_ARCTIC_EMBED_M: &'static str = "snowflake-arctic-embed:xs";
    const JINA_EMBEDDINGS_V2_BASE_ES: &'static str = "jina/jina-embeddings-v2-base-es:latest";
    const EMBEDDING_GEMMA_300_M: &'static str = "embeddinggemma:300m";

    pub fn from_string(s: &str) -> Result<Self, ShinkaiEmbeddingError> {
        match s {
            Self::ALL_MINI_LML6V2 => Ok(Self::AllMiniLML6v2),
            Self::SNOWFLAKE_ARCTIC_EMBED_M => Ok(Self::SnowflakeArcticEmbedM),
            Self::JINA_EMBEDDINGS_V2_BASE_ES => Ok(Self::JinaEmbeddingsV2BaseEs),
            Self::EMBEDDING_GEMMA_300_M => Ok(Self::EmbeddingGemma300M),
            _ => Err(ShinkaiEmbeddingError::InvalidModelArchitecture),
        }
    }

    pub fn max_input_token_count(&self) -> usize {
        match self {
            Self::JinaEmbeddingsV2BaseEs => 1024,
            Self::EmbeddingGemma300M => 2048,
            _ => 512,
        }
    }

    pub fn embedding_normalization_factor(&self) -> f32 {
        match self {
            Self::JinaEmbeddingsV2BaseEs => 1.5,
            _ => 1.0,
        }
    }

    pub fn vector_dimensions(&self) -> Result<usize, ShinkaiEmbeddingError> {
        match self {
            Self::SnowflakeArcticEmbedM => Ok(384),
            Self::JinaEmbeddingsV2BaseEs => Ok(768),
            Self::EmbeddingGemma300M => Ok(768),
            _ => Err(ShinkaiEmbeddingError::UnimplementedModelDimensions(format!(
                "{:?}",
                self
            ))),
        }
    }
}

impl fmt::Display for OllamaTextEmbeddingsInference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::AllMiniLML6v2 => write!(f, "{}", Self::ALL_MINI_LML6V2),
            Self::SnowflakeArcticEmbedM => write!(f, "{}", Self::SNOWFLAKE_ARCTIC_EMBED_M),
            Self::JinaEmbeddingsV2BaseEs => write!(f, "{}", Self::JINA_EMBEDDINGS_V2_BASE_ES),
            Self::EmbeddingGemma300M => write!(f, "{}", Self::EMBEDDING_GEMMA_300_M),
            Self::Other(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_snowflake_arctic_embed_xs() {
        let model_str = "snowflake-arctic-embed:xs";
        let parsed_model = OllamaTextEmbeddingsInference::from_string(model_str);
        assert_eq!(parsed_model, Ok(OllamaTextEmbeddingsInference::EmbeddingGemma300M));
    }

    #[test]
    fn test_parse_jina_embeddings_v2_base_es() {
        let model_str = "jina/jina-embeddings-v2-base-es:latest";
        let parsed_model = OllamaTextEmbeddingsInference::from_string(model_str);
        assert_eq!(parsed_model, Ok(OllamaTextEmbeddingsInference::JinaEmbeddingsV2BaseEs));
    }

    #[test]
    fn test_parse_embedding_gemma_300m() {
        let model_str = "embeddinggemma:300m";
        let parsed_model = OllamaTextEmbeddingsInference::from_string(model_str);
        assert_eq!(parsed_model, Ok(OllamaTextEmbeddingsInference::EmbeddingGemma300M));
    }

    #[test]
    fn test_parse_snowflake_arctic_embed_xs_as_embedding_model_type() {
        let model_str = "snowflake-arctic-embed:xs";
        let parsed_model = EmbeddingModelType::from_string(model_str);
        assert_eq!(
            parsed_model,
            Ok(EmbeddingModelType::OllamaTextEmbeddingsInference(
                OllamaTextEmbeddingsInference::EmbeddingGemma300M
            ))
        );
    }

    #[test]
    fn test_parse_jina_embeddings_v2_base_es_as_embedding_model_type() {
        let model_str = "jina/jina-embeddings-v2-base-es:latest";
        let parsed_model = EmbeddingModelType::from_string(model_str);
        assert_eq!(
            parsed_model,
            Ok(EmbeddingModelType::OllamaTextEmbeddingsInference(
                OllamaTextEmbeddingsInference::JinaEmbeddingsV2BaseEs
            ))
        );
    }

    #[test]
    fn test_parse_embedding_gemma_300m_as_embedding_model_type() {
        let model_str = "embeddinggemma:300m";
        let parsed_model = EmbeddingModelType::from_string(model_str);
        assert_eq!(
            parsed_model,
            Ok(EmbeddingModelType::OllamaTextEmbeddingsInference(
                OllamaTextEmbeddingsInference::EmbeddingGemma300M
            ))
        );
    }
}
