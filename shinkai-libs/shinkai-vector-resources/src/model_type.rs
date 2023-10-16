// pub use llm::ModelArchitecture;
use std::fmt;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingModelType {
    TextEmbeddingsInference(TextEmbeddingsInference),
    BertCPP(BertCPP),
    OpenAI(OpenAI),
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TextEmbeddingsInference {
    AllMiniLML6v2,
    AllMiniLML12v2,
    MultiQAMiniLML6,
    Other(String),
}

/// Bert.CPP (https://github.com/skeskinen/bert.cpp)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BertCPP {
    AllMiniLML6v2,
    AllMiniLML12v2,
    MultiQAMiniLML6,
    Other(String),
}

/// OpenAI
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OpenAI {
    OpenAITextEmbeddingAda002,
}

impl fmt::Display for TextEmbeddingsInference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TextEmbeddingsInference::AllMiniLML6v2 => write!(f, "all-MiniLM-L6-v2"),
            TextEmbeddingsInference::AllMiniLML12v2 => write!(f, "all-MiniLM-L12-v2"),
            TextEmbeddingsInference::MultiQAMiniLML6 => write!(f, "multi-qa-MiniLM-L6-cos-v1"),
            TextEmbeddingsInference::Other(name) => write!(f, "{}", name),
        }
    }
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
