use crate::resources::embeddings::*;

pub struct DataChunk {
    pub id: String,
    pub data: String,
}

pub trait Resource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> Option<&str>;
    fn resource_embedding(&self) -> &Embedding;

    fn new(
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        resource_embedding: Embedding,
        chunk_embeddings: Vec<Embedding>,
    ) -> Self;

    // Method to retrieve data chunk
    fn get_data_chunk(&self, id: String) -> DataChunk;
}
