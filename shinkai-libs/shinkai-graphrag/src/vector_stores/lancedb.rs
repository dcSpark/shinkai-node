use std::{collections::HashMap, sync::Arc};

use arrow::datatypes::Float32Type;
use arrow_array::{Array, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray};
use futures::TryStreamExt;
use lancedb::{
    arrow::arrow_schema::{DataType, Field, Schema},
    connect,
    query::{ExecutableQuery, QueryBase},
    Connection, Table,
};
use serde_json::json;

use crate::llm::base::BaseTextEmbedding;

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
        query_embedding: Vec<f32>,
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
                    .downcast_ref::<FixedSizeListArray>()
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
                    .downcast_ref::<Float32Array>()
                    .unwrap();

                if id_col.is_empty()
                    || text_col.is_empty()
                    || vector_col.is_empty()
                    || attributes_col.is_empty()
                    || distance_col.is_empty()
                {
                    continue;
                }

                for i in 0..record.num_rows() {
                    let id = id_col.value(i).to_string();
                    let text = text_col.value(i).to_string();
                    let vector: Vec<f32> = vector_col
                        .value(i)
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .iter()
                        .map(|value| value.unwrap())
                        .collect();
                    let attributes: HashMap<String, String> = serde_json::from_str(attributes_col.value(i))?;

                    let distance = distance_col.value(i);

                    let score = 1.0 - distance.abs();

                    let doc = VectorStoreDocument {
                        id,
                        text: Some(text),
                        vector: Some(vector),
                        attributes,
                    };

                    results.push(VectorStoreSearchResult { document: doc, score });
                }
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
        let query_embedding = text_embedder.aembed(text).await?;

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

        let vector_dimension = if !data.is_empty() {
            data[0].vector.as_ref().map(|v| v.len()).unwrap_or_default()
        } else {
            0
        };

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dimension.try_into().unwrap(),
                ),
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
                    Arc::new(FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        data.iter()
                            .map(|document| {
                                Some(
                                    document
                                        .vector
                                        .as_ref()
                                        .map(|v| v.iter().map(|f| Some(*f)).collect::<Vec<_>>())
                                        .unwrap_or_default(),
                                )
                            })
                            .collect::<Vec<_>>(),
                        vector_dimension.try_into().unwrap(),
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
            let _ = db_connection.drop_table(&self.collection_name).await;

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
            let table = match db_connection.open_table(&self.collection_name).execute().await {
                Ok(table) => table,
                Err(_) => {
                    let table = db_connection
                        .create_empty_table(&self.collection_name, schema.clone())
                        .execute()
                        .await?;

                    table
                }
            };

            if let Some(batches) = batches {
                table.add(batches).execute().await?;
            }

            self.document_collection = Some(table);
        }

        Ok(())
    }
}
