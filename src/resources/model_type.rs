pub use llm::ModelArchitecture;
use std::fmt;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingModelType {
    LocalModel(LocalModel),
    RemoteModel(RemoteModel),
}

impl fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EmbeddingModelType::LocalModel(local_model) => write!(f, "{}", local_model),
            EmbeddingModelType::RemoteModel(remote_model) => write!(f, "{}", remote_model),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LocalModel {
    Bloom,
    Gpt2,
    GptJ,
    GptNeoX,
    Llama,
    Mpt,
    Falcon,
}

impl LocalModel {
    pub fn from_model_architecture(arch: ModelArchitecture) -> LocalModel {
        match arch {
            ModelArchitecture::Bloom => LocalModel::Bloom,
            ModelArchitecture::Gpt2 => LocalModel::Gpt2,
            ModelArchitecture::GptJ => LocalModel::GptJ,
            ModelArchitecture::GptNeoX => LocalModel::GptNeoX,
            ModelArchitecture::Llama => LocalModel::Llama,
            ModelArchitecture::Mpt => LocalModel::Mpt,
            //ModelArchitecture::Falcon => LocalModel::Falcon, // Falcon not implemented yet in llm crate
            _ => LocalModel::Llama,
        }
    }
}

impl fmt::Display for LocalModel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LocalModel::Bloom => write!(f, "Bloom"),
            LocalModel::Gpt2 => write!(f, "Gpt2"),
            LocalModel::GptJ => write!(f, "GptJ"),
            LocalModel::GptNeoX => write!(f, "GptNeoX"),
            LocalModel::Llama => write!(f, "Llama"),
            LocalModel::Mpt => write!(f, "Mpt"),
            LocalModel::Falcon => write!(f, "Falcon"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RemoteModel {
    OpenAITextEmbeddingAda002,
    Other(String),
}

impl fmt::Display for RemoteModel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RemoteModel::OpenAITextEmbeddingAda002 => write!(f, "text-embedding-ada-002"),
            RemoteModel::Other(name) => write!(f, "{}", name),
        }
    }
}
