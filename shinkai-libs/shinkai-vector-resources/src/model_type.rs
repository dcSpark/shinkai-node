use crate::resource_errors::VRError;
use serde::{Deserialize, Deserializer, Serialize, Serializer}; // pub use llm::ModelArchitecture;
use std::fmt;
use std::hash::{Hash, Hasher};

// Alias for embedding model type string
pub type EmbeddingModelTypeString = String;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum EmbeddingModelType {
    TextEmbeddingsInference(TextEmbeddingsInference),
    OpenAI(OpenAIModelType),
}

impl EmbeddingModelType {
    /// Converts the embedding model type to a string
    pub fn to_string(&self) -> String {
        match self {
            EmbeddingModelType::TextEmbeddingsInference(model) => model.to_string(),
            EmbeddingModelType::OpenAI(model) => model.to_string(),
        }
    }

    /// Parses a string into an embedding model type
    pub fn from_string(s: &str) -> Result<Self, VRError> {
        if let Ok(model) = TextEmbeddingsInference::from_string(s) {
            return Ok(EmbeddingModelType::TextEmbeddingsInference(model));
        }
        if let Ok(model) = OpenAIModelType::from_string(s) {
            return Ok(EmbeddingModelType::OpenAI(model));
        }
        Err(VRError::InvalidModelArchitecture)
    }

    /// Returns the maximum allowed token count for an input string to be embedded, based on the embedding model
    pub fn max_input_token_count(&self) -> usize {
        match self {
            EmbeddingModelType::TextEmbeddingsInference(model) => match model {
                TextEmbeddingsInference::AllMiniLML6v2 => 510,
                TextEmbeddingsInference::AllMiniLML12v2 => 510,
                TextEmbeddingsInference::MultiQAMiniLML6 => 510,
                TextEmbeddingsInference::BgeLargeEnv1_5 => 510,
                TextEmbeddingsInference::BgeBaseEn1_5 => 510,
                TextEmbeddingsInference::EmberV1 => 510,
                TextEmbeddingsInference::GteLarge => 510,
                TextEmbeddingsInference::GteBase => 510,
                TextEmbeddingsInference::E5LargeV2 => 510,
                TextEmbeddingsInference::BgeSmallEn1_5 => 510,
                TextEmbeddingsInference::E5BaseV2 => 510,
                TextEmbeddingsInference::MultilingualE5Large => 510,
                TextEmbeddingsInference::Other(_) => 510,
            },
            EmbeddingModelType::OpenAI(model) => match model {
                OpenAIModelType::OpenAITextEmbeddingAda002 => 8190,
            },
        }
    }
}

impl fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EmbeddingModelType::TextEmbeddingsInference(model) => model.to_string().fmt(f),
            EmbeddingModelType::OpenAI(model) => model.to_string().fmt(f),
        }
    }
}

/// Hugging Face's Text Embeddings Inference Server
/// (https://github.com/huggingface/text-embeddings-inference)
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TextEmbeddingsInference {
    AllMiniLML6v2,
    AllMiniLML12v2,
    MultiQAMiniLML6,
    BgeLargeEnv1_5,
    BgeBaseEn1_5,
    EmberV1,
    GteLarge,
    GteBase,
    E5LargeV2,
    BgeSmallEn1_5,
    E5BaseV2,
    MultilingualE5Large,
    Other(String),
}
impl TextEmbeddingsInference {
    const ALL_MINI_LML6V2: &'static str = "hftei/sentence-transformers/all-MiniLM-L6-v2";
    const ALL_MINI_LML12V2: &'static str = "hftei/sentence-transformers/all-MiniLM-L12-v2";
    const MULTI_QA_MINI_LML6: &'static str = "hftei/sentence-transformers/multi-qa-MiniLM-L6-cos-v1";
    const BGE_LARGE_ENV1_5: &'static str = "hftei/BAAI/bge-large-en-v1.5";
    const BGE_BASE_EN1_5: &'static str = "hftei/BAAI/bge-base-en-v1.5";
    const BGE_SMALL_EN1_5: &'static str = "hftei/BAAI/bge-small-en-v1.5";
    const EMBER_V1: &'static str = "hftei/llmrails/ember-v1";
    const GTE_LARGE: &'static str = "hftei/thenlper/gte-large";
    const GTE_BASE: &'static str = "hftei/thenlper/gte-base";
    const E5_LARGE_V2: &'static str = "hftei/intfloat/e5-large-v2";
    const E5_BASE_V2: &'static str = "hftei/intfloat/e5-base-v2";
    const MULTILINGUAL_E5_LARGE: &'static str = "hftei/intfloat/multilingual-e5-large";

    fn to_string(&self) -> String {
        match self {
            TextEmbeddingsInference::AllMiniLML6v2 => Self::ALL_MINI_LML6V2.to_string(),
            TextEmbeddingsInference::AllMiniLML12v2 => Self::ALL_MINI_LML12V2.to_string(),
            TextEmbeddingsInference::MultiQAMiniLML6 => Self::MULTI_QA_MINI_LML6.to_string(),
            TextEmbeddingsInference::BgeLargeEnv1_5 => Self::BGE_LARGE_ENV1_5.to_string(),
            TextEmbeddingsInference::BgeBaseEn1_5 => Self::BGE_BASE_EN1_5.to_string(),
            TextEmbeddingsInference::BgeSmallEn1_5 => Self::BGE_SMALL_EN1_5.to_string(),
            TextEmbeddingsInference::EmberV1 => Self::EMBER_V1.to_string(),
            TextEmbeddingsInference::GteLarge => Self::GTE_LARGE.to_string(),
            TextEmbeddingsInference::GteBase => Self::GTE_BASE.to_string(),
            TextEmbeddingsInference::E5LargeV2 => Self::E5_LARGE_V2.to_string(),
            TextEmbeddingsInference::E5BaseV2 => Self::E5_BASE_V2.to_string(),
            TextEmbeddingsInference::MultilingualE5Large => Self::MULTILINGUAL_E5_LARGE.to_string(),
            TextEmbeddingsInference::Other(name) => format!("hftei/{}", name),
        }
    }

    fn from_string(s: &str) -> Result<Self, VRError> {
        match s {
            Self::ALL_MINI_LML6V2 => Ok(TextEmbeddingsInference::AllMiniLML6v2),
            Self::ALL_MINI_LML12V2 => Ok(TextEmbeddingsInference::AllMiniLML12v2),
            Self::MULTI_QA_MINI_LML6 => Ok(TextEmbeddingsInference::MultiQAMiniLML6),
            Self::BGE_LARGE_ENV1_5 => Ok(TextEmbeddingsInference::BgeLargeEnv1_5),
            Self::BGE_BASE_EN1_5 => Ok(TextEmbeddingsInference::BgeBaseEn1_5),
            Self::BGE_SMALL_EN1_5 => Ok(TextEmbeddingsInference::BgeSmallEn1_5),
            Self::EMBER_V1 => Ok(TextEmbeddingsInference::EmberV1),
            Self::GTE_LARGE => Ok(TextEmbeddingsInference::GteLarge),
            Self::GTE_BASE => Ok(TextEmbeddingsInference::GteBase),
            Self::E5_LARGE_V2 => Ok(TextEmbeddingsInference::E5LargeV2),
            Self::E5_BASE_V2 => Ok(TextEmbeddingsInference::E5BaseV2),
            Self::MULTILINGUAL_E5_LARGE => Ok(TextEmbeddingsInference::MultilingualE5Large),
            _ => Err(VRError::InvalidModelArchitecture),
        }
    }
}

impl fmt::Display for TextEmbeddingsInference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// OpenAIModelType
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OpenAIModelType {
    OpenAITextEmbeddingAda002,
}

impl OpenAIModelType {
    const OPENAI_TEXT_EMBEDDING_ADA_002: &'static str = "openai/text-embedding-ada-002";

    fn to_string(&self) -> String {
        match self {
            OpenAIModelType::OpenAITextEmbeddingAda002 => Self::OPENAI_TEXT_EMBEDDING_ADA_002.to_string(),
        }
    }

    fn from_string(s: &str) -> Result<OpenAIModelType, VRError> {
        match s {
            Self::OPENAI_TEXT_EMBEDDING_ADA_002 => Ok(OpenAIModelType::OpenAITextEmbeddingAda002),
            _ => Err(VRError::InvalidModelArchitecture),
        }
    }
}

impl fmt::Display for OpenAIModelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
