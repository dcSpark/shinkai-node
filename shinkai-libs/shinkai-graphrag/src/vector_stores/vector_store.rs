use std::collections::HashMap;

pub struct VectorStoreSearchResult {
    pub document: VectorStoreDocument,
    pub score: f64,
}

pub struct VectorStoreDocument {
    pub id: String,
    pub text: Option<String>,
    pub attributes: HashMap<String, String>,
}

pub trait VectorStore {
    fn similarity_search_by_text(
        &self,
        text: &str,
        text_embedder: &dyn Fn(&str) -> Vec<f64>,
        k: usize,
    ) -> Vec<VectorStoreSearchResult>;
}
