use super::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError};

use arrow_array::Array;
use arrow_array::{RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field};
use futures::TryStreamExt;
use lancedb::query::ExecutableQuery;
use lancedb::query::QueryBase;
use lancedb::Table;
use lancedb::{query::Select, table::AddDataMode, Connection, Error as LanceDbError};
use std::sync::Arc;

impl LanceShinkaiDb {
    pub async fn create_version_table(connection: &Connection) -> Result<Table, ShinkaiLanceDBError> {
        let schema = arrow_schema::Schema::new(vec![Field::new("version", DataType::Utf8, false)]);

        match connection
            .create_empty_table("version", schema.into())
            // .data_storage_version(LanceFileVersion::V2_1)
            .execute()
            .await
        {
            Ok(table) => Ok(table),
            Err(LanceDbError::TableAlreadyExists { .. }) => connection
                .open_table("version")
                .execute()
                .await
                .map_err(ShinkaiLanceDBError::from),
            Err(e) => Err(ShinkaiLanceDBError::from(e)),
        }
    }

    pub async fn get_current_version(&self) -> Result<Option<String>, ShinkaiLanceDBError> {
        let query = self
            .version_table
            .query()
            .select(Select::columns(&["version"]))
            .limit(1)
            .execute()
            .await?;

        let results = query.try_collect::<Vec<_>>().await?;
        if let Some(batch) = results.first() {
            let version_array = batch
                .column_by_name("version")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            if version_array.len() > 0 {
                return Ok(Some(version_array.value(0).to_string()));
            }
        }
        Ok(None)
    }

    pub async fn set_version(&self, version: &str) -> Result<(), ShinkaiLanceDBError> {
        let schema = self.version_table.schema().await.map_err(ShinkaiLanceDBError::from)?;
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(StringArray::from(vec![version.to_string()]))],
        )
        .map_err(|e| ShinkaiLanceDBError::Arrow(e.to_string()))?;
        let batch_reader = Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema.clone()));
        self.version_table
            .add(batch_reader)
            .mode(AddDataMode::Append)
            .execute()
            .await
            .map_err(ShinkaiLanceDBError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use std::fs;
    use std::path::Path;

    fn setup() {
        let path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_version_management() -> Result<(), ShinkaiLanceDBError> {
        
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        // Try to read the current version (should return None)
        let current_version = db.get_current_version().await?;
        assert!(current_version.is_none(), "Initial version should be None");

        // Set the version to "1"
        db.set_version("1").await?;

        // Read the version again (should return "1")
        let current_version = db.get_current_version().await?;
        assert_eq!(current_version, Some("1".to_string()), "Version should be '1'");

        Ok(())
    }
}
