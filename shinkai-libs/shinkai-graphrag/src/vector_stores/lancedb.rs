use std::{collections::HashMap, sync::Arc};

use arrow::datatypes::Float64Type;
use arrow_array::{Array, Float64Array, ListArray, RecordBatch, RecordBatchIterator, StringArray};
use futures::TryStreamExt;
use lancedb::{
    arrow::arrow_schema::{DataType, Field, Schema},
    connect,
    query::{ExecutableQuery, QueryBase},
    Connection, Table,
};
use serde_json::json;

use crate::llm::llm::BaseTextEmbedding;

use super::vector_store::{VectorStore, VectorStoreDocument, VectorStoreSearchResult};

pub struct LanceDBVectorStore {
    collection_name: String,
    db_connection: Option<Connection>,
    document_collection: Option<Table>,
}

impl LanceDBVectorStore {
    pub fn new(collection_name: String) -> Self {
        LanceDBVectorStore {
            collection_name,
            db_connection: None,
            document_collection: None,
        }
    }

    pub async fn connect(&mut self, db_uri: &str) -> anyhow::Result<()> {
        let connection = connect(db_uri).execute().await?;
        self.db_connection = Some(connection);
        Ok(())
    }

    async fn similarity_search_by_vector(
        &self,
        query_embedding: Vec<f64>,
        k: usize,
    ) -> anyhow::Result<Vec<VectorStoreSearchResult>> {
        if let Some(document_collection) = &self.document_collection {
            let records = document_collection
                .query()
                .limit(k)
                .nearest_to(query_embedding)?
                .execute()
                .await?
                .try_collect::<Vec<_>>()
                .await?;

            let mut results = Vec::new();
            for record in records {
                let id_col = record
                    .column_by_name("id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                let text_col = record
                    .column_by_name("text")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                let vector_col = record
                    .column_by_name("vector")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<ListArray>()
                    .unwrap();
                let attributes_col = record
                    .column_by_name("attributes")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();

                let distance_col = record
                    .column_by_name("_distance")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap();

                if id_col.len() == 0
                    || text_col.len() == 0
                    || vector_col.len() == 0
                    || attributes_col.len() == 0
                    || distance_col.len() == 0
                {
                    continue;
                }

                let id = id_col.value(0).to_string();
                let text = text_col.value(0).to_string();
                let vector: Vec<f64> = vector_col
                    .value(0)
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap()
                    .iter()
                    .map(|value| value.unwrap())
                    .collect();
                let attributes: HashMap<String, String> = serde_json::from_str(attributes_col.value(0))?;

                let distance = distance_col.value(0);

                let score = 1.0 - distance.abs();

                let doc = VectorStoreDocument {
                    id,
                    text: Some(text),
                    vector: Some(vector),
                    attributes,
                };

                results.push(VectorStoreSearchResult { document: doc, score });
            }

            return Ok(results);
        }

        Ok(Vec::new())
    }
}

impl VectorStore for LanceDBVectorStore {
    async fn similarity_search_by_text(
        &self,
        text: &str,
        text_embedder: &Box<dyn BaseTextEmbedding + Send + Sync>,
        k: usize,
    ) -> anyhow::Result<Vec<VectorStoreSearchResult>> {
        let query_embedding = text_embedder.embed(text);

        if query_embedding.is_empty() {
            return Ok(vec![]);
        }

        let results = self.similarity_search_by_vector(query_embedding, k).await?;
        Ok(results)
    }

    async fn load_documents(&mut self, documents: Vec<VectorStoreDocument>, overwrite: bool) -> anyhow::Result<()> {
        let db_connection = self
            .db_connection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LanceDB connection is not established"))?;

        let data: Vec<_> = documents
            .into_iter()
            .filter(|document| document.vector.is_some())
            .collect();

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, true),
            Field::new(
                "vector",
                DataType::List(Arc::new(Field::new("item", DataType::Float64, true))),
                true,
            ),
            Field::new("attributes", DataType::Utf8, false),
        ]));

        let batches = if !data.is_empty() {
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(StringArray::from(
                        data.iter().map(|document| document.id.clone()).collect::<Vec<_>>(),
                    )),
                    Arc::new(StringArray::from(
                        data.iter()
                            .map(|document| document.text.clone().unwrap_or_default())
                            .collect::<Vec<_>>(),
                    )),
                    Arc::new(ListArray::from_iter_primitive::<Float64Type, _, _>(
                        data.iter()
                            .map(|document| {
                                Some(
                                    document
                                        .vector
                                        .as_ref()
                                        .map(|v| v.iter().map(|f| Some(f.clone())).collect::<Vec<_>>())
                                        .unwrap_or_default(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )),
                    Arc::new(StringArray::from(
                        data.iter()
                            .map(|document| json!(document.attributes).to_string())
                            .collect::<Vec<_>>(),
                    )),
                ],
            )?;

            Some(RecordBatchIterator::new(vec![Ok(batch)], schema.clone()))
        } else {
            None
        };

        if overwrite {
            if let Some(batches) = batches {
                let table = db_connection
                    .create_table(&self.collection_name, Box::new(batches))
                    .execute()
                    .await?;

                self.document_collection = Some(table);
            } else {
                let table = db_connection
                    .create_empty_table(&self.collection_name, schema.clone())
                    .execute()
                    .await?;

                self.document_collection = Some(table);
            }
        } else {
            let table = db_connection.open_table(&self.collection_name).execute().await?;

            if let Some(batches) = batches {
                table.add(batches).execute().await?;
            }

            self.document_collection = Some(table);
        }

        Ok(())
    }
}
