use std::collections::HashMap;
use crate::errors::SqliteManagerError;
use crate::SqliteManager;
use crate::preferences::serde_json::Value;
use rusqlite::{OptionalExtension, Result, ToSql};
use serde;
use serde_json;

impl SqliteManager {
    /// Initializes the preferences table in the database.
    /// Creates a table that stores key-value pairs with metadata including descriptions and timestamps.
    pub fn initialize_preferences_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS preferences (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL, -- Store as JSON string for flexible schema
                description TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );",
            [],
        )?;

        // Create an index for faster key lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_preferences_key ON preferences (key);",
            [],
        )?;

        Ok(())
    }

    /// Stores a preference value in the database.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the preference
    /// * `value` - The value to store. Can be any type that implements serde::Serialize. Complex types (like structs)
    ///   will be serialized to JSON.
    /// * `description` - Optional description of what this preference is used for
    pub fn set_preference<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        description: Option<&str>,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let json_value = serde_json::to_string(value)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        conn.execute(
            "INSERT INTO preferences (key, value, description, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET 
             value = ?2,
             description = ?3,
             updated_at = CURRENT_TIMESTAMP",
            [
                &key as &dyn ToSql,
                &json_value as &dyn ToSql,
                &description as &dyn ToSql,
            ],
        )?;

        Ok(())
    }

    /// Retrieves a preference value from the database.
    ///
    /// # Arguments
    /// * `key` - The unique identifier of the preference to retrieve
    ///
    /// # Returns
    /// Returns `Ok(Some(T))` if the preference exists and can be deserialized to type T,
    /// `Ok(None)` if the preference doesn't exist, or an error if deserialization fails.
    pub fn get_preference<T: serde::de::DeserializeOwned + Sized>(
        &self,
        key: &str,
    ) -> Result<Option<T>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let result: Option<String> = conn
            .query_row("SELECT value FROM preferences WHERE key = ?1", [key], |row| row.get(0))
            .optional()?;

        match result {
            Some(json_value) => {
                let value = serde_json::from_str(&json_value)
                    .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Deletes a preference from the database.
    ///
    /// # Arguments
    /// * `key` - The unique identifier of the preference to delete
    ///
    /// # Returns
    /// Returns `true` if a preference was deleted, `false` if no preference with that key existed.
    pub fn delete_preference(&self, key: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute("DELETE FROM preferences WHERE key = ?1", [key])?;
        Ok(rows_affected > 0)
    }

    /// Lists all preferences stored in the database.
    ///
    /// # Returns
    /// Returns a vector of tuples containing:
    /// * key (String)
    /// * value (String) - JSON-serialized value
    /// * description (Option<String>)
    /// * updated_at (String) - Timestamp of last update
    ///
    /// The results are ordered by key.
    pub fn list_preferences(&self) -> Result<Vec<(String, String, Option<String>, String)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT key, value, description, updated_at FROM preferences ORDER BY key")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;

        let mut preferences = Vec::new();
        for row in rows {
            preferences.push(row?);
        }
        Ok(preferences)
    }

    /// Returns a `Result` containing a `HashMap` where keys are preference names (String)
    /// and values are `serde_json::Value`, or an `SqliteManagerError` on failure.
    pub fn get_all_preferences(&self) -> Result<HashMap<String, serde_json::Value>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT key, value FROM preferences ORDER BY key")?;
        let preferences_iter = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let value_str: String = row.get(1)?;
            // Parse the JSON string into a serde_json::Value
            let value: serde_json::Value = serde_json::from_str(&value_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    1, // Column index (value)
                    rusqlite::types::Type::Text, // Expected type from DB (JSON stored as TEXT)
                    Box::new(e) // The actual serde_json error
                ))?;
            Ok((key, value))
        })?;

        // Collect the iterator of results into a single result containing the HashMap
        let preferences = preferences_iter.collect::<Result<HashMap<String, serde_json::Value>, rusqlite::Error>>()?;
        Ok(preferences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteManager;
    use serde::{Deserialize, Serialize};
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    // Test structs
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        number: i32,
        flag: bool,
        nested: TestNestedConfig,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestNestedConfig {
        value: f64,
        tags: Vec<String>,
    }

    #[tokio::test]
    async fn test_preferences_crud_operations() {
        let manager = setup_test_db().await;

        // Test setting and getting a simple string preference
        manager
            .set_preference("test_string", &"hello world", Some("A test string"))
            .unwrap();
        let result: Option<String> = manager.get_preference("test_string").unwrap();
        assert_eq!(result, Some("hello world".to_string()));

        // Test setting and getting a complex struct
        let test_config = TestConfig {
            name: "test".to_string(),
            number: 42,
            flag: true,
            nested: TestNestedConfig {
                value: 3.14,
                tags: vec!["tag1".to_string(), "tag2".to_string()],
            },
        };
        manager
            .set_preference("test_config", &test_config, Some("A test configuration"))
            .unwrap();
        let result: Option<TestConfig> = manager.get_preference("test_config").unwrap();
        assert_eq!(result, Some(test_config));

        // Test updating an existing preference
        manager
            .set_preference("test_string", &"updated value", Some("Updated description"))
            .unwrap();
        let result: Option<String> = manager.get_preference("test_string").unwrap();
        assert_eq!(result, Some("updated value".to_string()));

        // Test getting a non-existent preference
        let result: Option<String> = manager.get_preference("non_existent").unwrap();
        assert_eq!(result, None);

        // Test deleting a preference
        assert!(manager.delete_preference("test_string").unwrap());
        let result: Option<String> = manager.get_preference("test_string").unwrap();
        assert_eq!(result, None);

        // Test deleting a non-existent preference
        assert!(!manager.delete_preference("non_existent").unwrap());
    }

    #[tokio::test]
    async fn test_preferences_list_and_metadata() {
        let manager = setup_test_db().await;

        // Add some test preferences
        manager
            .set_preference("pref1", &"value1", Some("Description 1"))
            .unwrap();
        manager
            .set_preference("pref2", &"value2", Some("Description 2"))
            .unwrap();
        manager.set_preference("pref3", &"value3", None).unwrap();

        // Test listing preferences
        let preferences = manager.list_preferences().unwrap();
        assert_eq!(preferences.len(), 3);

        // Verify preferences are ordered by key
        assert_eq!(preferences[0].0, "pref1");
        assert_eq!(preferences[1].0, "pref2");
        assert_eq!(preferences[2].0, "pref3");

        // Verify values and descriptions
        assert_eq!(preferences[0].1, "\"value1\""); // JSON string representation
        assert_eq!(preferences[0].2, Some("Description 1".to_string()));
        assert_eq!(preferences[2].2, None);

        // Verify timestamps are present
        assert!(!preferences[0].3.is_empty());
    }

    #[tokio::test]
    async fn test_preferences_concurrent_access() {
        let manager = Arc::new(setup_test_db().await);
        let mut handles = vec![];

        // Spawn multiple threads to write different preferences
        for i in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                manager_clone
                    .set_preference(&key, &value, Some(&format!("Description {}", i)))
                    .unwrap();
            });
            handles.push(handle);
        }

        // Wait for all writes to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all preferences were written correctly
        let preferences = manager.list_preferences().unwrap();
        assert_eq!(preferences.len(), 10);

        // Test concurrent reads and writes
        let manager = Arc::new(setup_test_db().await);
        let mut handles = vec![];

        // Set initial value
        manager
            .set_preference("concurrent_key", &"initial", Some("Test concurrent access"))
            .unwrap();

        // Spawn threads that read and write simultaneously
        for i in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                // Write operation
                manager_clone
                    .set_preference("concurrent_key", &format!("value{}", i), None)
                    .unwrap();
                // Read operation
                let _: Option<String> = manager_clone.get_preference("concurrent_key").unwrap();
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify the final state is valid
        let result: Option<String> = manager.get_preference("concurrent_key").unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_preferences_schema_evolution() {
        let manager = setup_test_db().await;

        // Original simple struct with just one field
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct OriginalConfig {
            name: String,
        }

        // Extended struct with an additional field
        // Note: The new field must have a default value for deserialization to work
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct ExtendedConfig {
            name: String,
            #[serde(default)] // This makes the field optional during deserialization
            count: i32,
        }

        // Store the original config
        let original = OriginalConfig {
            name: "test".to_string(),
        };
        manager
            .set_preference("evolving_config", &original, Some("Original config"))
            .unwrap();

        // Read it back as the extended config
        let extended: ExtendedConfig = manager.get_preference("evolving_config").unwrap().unwrap();

        // Verify the original field was preserved and the new field has the default value
        assert_eq!(extended.name, "test");
        assert_eq!(extended.count, 0); // Default value for i32

        // Now store the extended config
        let new_extended = ExtendedConfig {
            name: "test".to_string(),
            count: 42,
        };
        manager
            .set_preference("evolving_config", &new_extended, Some("Extended config"))
            .unwrap();

        // We can still read it as the original config, ignoring the extra field
        let original_from_extended: OriginalConfig = manager.get_preference("evolving_config").unwrap().unwrap();
        assert_eq!(original_from_extended.name, "test");

        // And we can read it as the extended config, getting all fields
        let final_extended: ExtendedConfig = manager.get_preference("evolving_config").unwrap().unwrap();
        assert_eq!(final_extended.name, "test");
        assert_eq!(final_extended.count, 42);
    }

    #[tokio::test]
    async fn test_get_all_preferences() {
        let manager = setup_test_db().await;

        // Set some preferences with different types
        manager.set_preference("key_string", &"value_string", None).unwrap();
        manager.set_preference("key_int", &123, None).unwrap();
        manager.set_preference("key_bool", &true, None).unwrap();

        // Call the function to test
        let all_prefs_result = manager.get_all_preferences();
        assert!(all_prefs_result.is_ok());
        let all_prefs = all_prefs_result.unwrap();

        // Assert the number of preferences retrieved
        assert_eq!(all_prefs.len(), 3);
        // Assert the values are correct JSON strings
        assert_eq!(all_prefs.get("key_string"), Some(&serde_json::json!("value_string")));
        assert_eq!(all_prefs.get("key_int"), Some(&serde_json::json!(123)));
        assert_eq!(all_prefs.get("key_bool"), Some(&serde_json::json!(true)));

        // Check for a non-existent key
        assert!(all_prefs.get("non_existent_key").is_none());
    }
}
