// pub use llm::ModelArchitecture;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingModelType {
    TextEmbeddingsInference(TextEmbeddingsInference),
    BertCPP(BertCPP),
    OpenAI(OpenAI),
}

impl EmbeddingModelType {
    /// Returns the maximum allowed token count for an input string to be embedded, based on the embedding model
    pub fn max_input_token_count(&self) -> usize {
        match self {
            EmbeddingModelType::TextEmbeddingsInference(model) => match model {
                TextEmbeddingsInference::AllMiniLML6v2 => 510,
                TextEmbeddingsInference::AllMiniLML12v2 => 510,
                TextEmbeddingsInference::MultiQAMiniLML6 => 510,
                TextEmbeddingsInference::BgeLargeEn => 510,
                TextEmbeddingsInference::BgeBaseEn => 510,
                TextEmbeddingsInference::EmberV1 => 510,
                TextEmbeddingsInference::GteLarge => 510,
                TextEmbeddingsInference::GteBase => 510,
                TextEmbeddingsInference::E5LargeV2 => 510,
                TextEmbeddingsInference::BgeSmallEn => 510,
                TextEmbeddingsInference::E5BaseV2 => 510,
                TextEmbeddingsInference::MultilingualE5Large => 510,
                TextEmbeddingsInference::Other(_) => 510,
            },
            EmbeddingModelType::BertCPP(model) => match model {
                BertCPP::AllMiniLML6v2 => 510,
                BertCPP::AllMiniLML12v2 => 510,
                BertCPP::MultiQAMiniLML6 => 510,
                BertCPP::Other(_) => 510,
            },
            EmbeddingModelType::OpenAI(model) => match model {
                OpenAI::OpenAITextEmbeddingAda002 => 8190,
            },
        }
    }
}

impl fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EmbeddingModelType::TextEmbeddingsInference(model) => write!(f, "{}", model),
            EmbeddingModelType::BertCPP(model) => write!(f, "{}", model),
            EmbeddingModelType::OpenAI(model) => write!(f, "{}", model),
        }
    }
}

/// Hugging Face's Text Embeddings Inference
/// (https://github.com/huggingface/text-embeddings-inference)
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TextEmbeddingsInference {
    AllMiniLML6v2,
    AllMiniLML12v2,
    MultiQAMiniLML6,
    BgeLargeEn,
    BgeBaseEn,
    EmberV1,
    GteLarge,
    GteBase,
    E5LargeV2,
    BgeSmallEn,
    E5BaseV2,
    MultilingualE5Large,
    Other(String),
}

impl fmt::Display for TextEmbeddingsInference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TextEmbeddingsInference::AllMiniLML6v2 => write!(f, "sentence-transformers/all-MiniLM-L6-v2"),
            TextEmbeddingsInference::AllMiniLML12v2 => write!(f, "sentence-transformers/all-MiniLM-L12-v2"),
            TextEmbeddingsInference::MultiQAMiniLML6 => write!(f, "sentence-transformers/multi-qa-MiniLM-L6-cos-v1"),
            TextEmbeddingsInference::BgeLargeEn => write!(f, "BAAI/bge-large-en-v1.5"),
            TextEmbeddingsInference::BgeBaseEn => write!(f, "BAAI/bge-base-en-v1.5"),
            TextEmbeddingsInference::EmberV1 => write!(f, "llmrails/ember-v1"),
            TextEmbeddingsInference::GteLarge => write!(f, "thenlper/gte-large"),
            TextEmbeddingsInference::GteBase => write!(f, "thenlper/gte-base"),
            TextEmbeddingsInference::E5LargeV2 => write!(f, "intfloat/e5-large-v2"),
            TextEmbeddingsInference::BgeSmallEn => write!(f, "BAAI/bge-small-en-v1.5"),
            TextEmbeddingsInference::E5BaseV2 => write!(f, "intfloat/e5-base-v2"),
            TextEmbeddingsInference::MultilingualE5Large => write!(f, "intfloat/multilingual-e5-large"),
            TextEmbeddingsInference::Other(name) => write!(f, "sentence-transformers/{}", name),
        }
    }
}

/// Bert.CPP (https://github.com/skeskinen/bert.cpp)
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BertCPP {
    AllMiniLML6v2,
    AllMiniLML12v2,
    MultiQAMiniLML6,
    Other(String),
}

/// OpenAI
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OpenAI {
    OpenAITextEmbeddingAda002,
}

impl fmt::Display for BertCPP {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BertCPP::AllMiniLML6v2 => write!(f, "all-MiniLM-L6-v2"),
            BertCPP::AllMiniLML12v2 => write!(f, "all-MiniLM-L12-v2"),
            BertCPP::MultiQAMiniLML6 => write!(f, "multi-qa-MiniLM-L6-cos-v1"),
            BertCPP::Other(name) => write!(f, "{}", name),
        }
    }
}

impl fmt::Display for OpenAI {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OpenAI::OpenAITextEmbeddingAda002 => write!(f, "text-embedding-ada-002"),
        }
    }
}
