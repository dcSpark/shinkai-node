use std::collections::HashMap;

use crate::llm::base::BaseTextEmbedding;

pub struct VectorStoreSearchResult {
    pub document: VectorStoreDocument,
    pub score: f32,
}

pub struct VectorStoreDocument {
    pub id: String,
    pub text: Option<String>,
    pub vector: Option<Vec<f32>>,
    pub attributes: HashMap<String, String>,
}

pub trait VectorStore {
    fn similarity_search_by_text(
        &self,
        text: &str,
        text_embedder: &(dyn BaseTextEmbedding + Send + Sync),
        k: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<VectorStoreSearchResult>>> + Send;

    fn load_documents(
        &mut self,
        documents: Vec<VectorStoreDocument>,
        overwrite: bool,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}
