pub use llm::ModelArchitecture;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingModelType {
    LocalModel(LocalModel),
    ExternalModel(ExternalModel),
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalModel {
    OpenAITextEmbeddingAda002,
}
