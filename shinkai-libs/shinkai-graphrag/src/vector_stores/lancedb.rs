use std::sync::Arc;

use arrow::datatypes::Float64Type;
use arrow_array::{FixedSizeListArray, Float64Array, RecordBatch, RecordBatchIterator, StringArray};
use lancedb::{
    arrow::arrow_schema::{DataType, Field, Schema},
    connect, Connection,
};
use serde_json::json;

use super::vector_store::{VectorStore, VectorStoreDocument, VectorStoreSearchResult};

pub struct LanceDBVectorStore {
    collection_name: String,
    db_connection: Option<Connection>,
}

impl LanceDBVectorStore {
    pub fn new(collection_name: String) -> Self {
        LanceDBVectorStore {
            collection_name,
            db_connection: None,
        }
    }

    pub async fn connect(&mut self, db_uri: &str) -> anyhow::Result<()> {
        let connection = connect(db_uri).execute().await?;
        self.db_connection = Some(connection);
        Ok(())
    }

    fn similarity_search_by_vector(&self, query_embedding: Vec<f64>, k: usize) -> Vec<VectorStoreSearchResult> {
        Vec::new()
    }
}

impl VectorStore for LanceDBVectorStore {
    fn similarity_search_by_text(
        &self,
        text: &str,
        text_embedder: &dyn Fn(&str) -> Vec<f64>,
        k: usize,
    ) -> Vec<VectorStoreSearchResult> {
        let query_embedding = text_embedder(text);

        if query_embedding.is_empty() {
            return vec![];
        }

        self.similarity_search_by_vector(query_embedding, k)
    }

    fn load_documents(&mut self, documents: Vec<VectorStoreDocument>, overwrite: bool) -> anyhow::Result<()> {
        let data: Vec<_> = documents
            .into_iter()
            .filter(|document| document.vector.is_some())
            .collect();

        let data = if data.is_empty() { None } else { Some(data) };

        let vector_len = data
            .as_ref()
            .and_then(|data| data.first())
            .and_then(|document| document.vector.as_ref())
            .map(|vector| vector.len())
            .unwrap_or_default();

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float64, false)),
                    vector_len.try_into().unwrap_or_default(),
                ),
                true,
            ),
            Field::new("attributes", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(
                    data.as_ref()
                        .map(|data| data.iter().map(|document| document.id.clone()).collect::<Vec<_>>())
                        .unwrap_or_default(),
                )),
                Arc::new(StringArray::from(
                    data.as_ref()
                        .map(|data| {
                            data.iter()
                                .map(|document| document.text.clone().unwrap_or_default())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                )),
                Arc::new(FixedSizeListArray::from_iter_primitive::<Float64Type, _, _>(
                    data.as_ref()
                        .map(|data| data.iter().map(|document| document.vector.clone()).collect::<Vec<_>>()),
                    vector_len.try_into().unwrap_or_default(),
                )),
                Arc::new(StringArray::from(
                    data.as_ref()
                        .map(|data| {
                            data.iter()
                                .map(|document| json!(document.attributes).to_string())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                )),
            ],
        );

        let batch_iterator = RecordBatchIterator::new(vec![batch], schema.clone());
        Ok(())
    }
}
