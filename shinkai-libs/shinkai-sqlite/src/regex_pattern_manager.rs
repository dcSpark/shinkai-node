use rusqlite::{Result, Row, ToSql, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{errors::SqliteManagerError, SqliteManager};

#[derive(Debug, Clone)]
pub struct RegexPattern {
    pub id: Option<i64>,
    provider_name: String,  // Made private to ensure it's set correctly
    pub pattern: String,
    pub response: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub priority: i32,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl RegexPattern {
    pub fn new(provider_name: String, pattern: String, response: String, description: Option<String>, priority: i32) -> Result<Self, SqliteManagerError> {
        // Basic validation
        if provider_name.trim().is_empty() {
            return Err(SqliteManagerError::ValidationError("Provider name cannot be empty".to_string()));
        }
        if pattern.trim().is_empty() {
            return Err(SqliteManagerError::ValidationError("Pattern cannot be empty".to_string()));
        }
        if response.trim().is_empty() {
            return Err(SqliteManagerError::ValidationError("Response cannot be empty".to_string()));
        }

        Ok(RegexPattern {
            id: None,
            provider_name,
            pattern,
            response,
            description,
            is_enabled: true,
            priority,
            created_at: None,
            updated_at: None,
        })
    }

    // Getter for provider_name since it's private
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    // Method to create a new pattern for a specific provider
    pub fn for_provider(
        provider_name: String,
        pattern: String,
        response: String,
        description: Option<String>,
        priority: i32,
    ) -> Result<Self, SqliteManagerError> {
        Self::new(provider_name, pattern, response, description, priority)
    }

    fn from_row(row: &Row<'_>) -> Result<Self> {
        Ok(RegexPattern {
            id: row.get("id")?,
            provider_name: row.get("provider_name")?,
            pattern: row.get("pattern")?,
            response: row.get("response")?,
            description: row.get("description")?,
            is_enabled: row.get("is_enabled")?,
            priority: row.get("priority")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

impl SqliteManager {
    pub fn add_regex_pattern(&self, pattern: &RegexPattern) -> Result<i64, SqliteManagerError> {
        // Additional validation before inserting
        if pattern.provider_name.trim().is_empty() {
            return Err(SqliteManagerError::ValidationError("Provider name cannot be empty".to_string()));
        }

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO regex_patterns (provider_name, pattern, response, description, is_enabled, priority)
             VALUES (?, ?, ?, ?, ?, ?)",
        )?;

        let id = stmt.insert([
            &pattern.provider_name as &dyn ToSql,
            &pattern.pattern as &dyn ToSql,
            &pattern.response as &dyn ToSql,
            &pattern.description as &dyn ToSql,
            &pattern.is_enabled as &dyn ToSql,
            &pattern.priority as &dyn ToSql,
        ])?;

        Ok(id)
    }

    pub fn get_regex_pattern(&self, id: i64) -> Result<Option<RegexPattern>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM regex_patterns WHERE id = ?")?;
        
        let pattern = stmt
            .query_row([id], RegexPattern::from_row)
            .optional()?;

        Ok(pattern)
    }

    pub fn get_enabled_regex_patterns_for_provider(&self, provider_name: &str) -> Result<Vec<RegexPattern>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM regex_patterns 
             WHERE provider_name = ? AND is_enabled = TRUE 
             ORDER BY priority DESC",
        )?;

        let patterns = stmt
            .query_map([provider_name], RegexPattern::from_row)?
            .collect::<Result<Vec<_>>>()?;

        Ok(patterns)
    }

    pub fn update_regex_pattern(&self, pattern: &RegexPattern) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "UPDATE regex_patterns 
             SET provider_name = ?, pattern = ?, response = ?, description = ?, is_enabled = ?, priority = ?
             WHERE id = ?",
        )?;

        stmt.execute([
            &pattern.provider_name as &dyn ToSql,
            &pattern.pattern as &dyn ToSql,
            &pattern.response as &dyn ToSql,
            &pattern.description as &dyn ToSql,
            &pattern.is_enabled as &dyn ToSql,
            &pattern.priority as &dyn ToSql,
            &pattern.id as &dyn ToSql,
        ])?;

        Ok(())
    }

    pub fn delete_regex_pattern(&self, id: i64) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM regex_patterns WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn get_all_regex_patterns(&self) -> Result<Vec<RegexPattern>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM regex_patterns ORDER BY priority DESC")?;

        let patterns = stmt
            .query_map([], RegexPattern::from_row)?
            .collect::<Result<Vec<_>>>()?;

        Ok(patterns)
    }

    pub fn get_all_regex_patterns_for_provider(&self, provider_name: &str) -> Result<Vec<RegexPattern>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM regex_patterns 
             WHERE provider_name = ? 
             ORDER BY priority DESC",
        )?;

        let patterns = stmt
            .query_map([provider_name], RegexPattern::from_row)?
            .collect::<Result<Vec<_>>>()?;

        Ok(patterns)
    }

    // Helper method to check if a provider exists
    pub fn provider_has_patterns(&self, provider_name: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM regex_patterns WHERE provider_name = ?",
            [provider_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_regex_pattern_crud() {
        let db = setup_test_db().await;

        // Create a pattern
        let pattern = RegexPattern::for_provider(
            "test_provider".to_string(),
            "hello.*world".to_string(),
            "Hello to you too!".to_string(),
            Some("Test pattern".to_string()),
            100,
        ).unwrap();

        // Add pattern
        let id = db.add_regex_pattern(&pattern).unwrap();

        // Get pattern
        let retrieved = db.get_regex_pattern(id).unwrap().unwrap();
        assert_eq!(retrieved.pattern, pattern.pattern);
        assert_eq!(retrieved.response, pattern.response);
        assert_eq!(retrieved.provider_name(), pattern.provider_name());

        // Test validation
        let invalid_pattern = RegexPattern::for_provider(
            "".to_string(),
            "hello.*world".to_string(),
            "Hello!".to_string(),
            None,
            100,
        );
        assert!(invalid_pattern.is_err());

        // Update pattern
        let mut updated = retrieved;
        updated.response = "Updated response".to_string();
        db.update_regex_pattern(&updated).unwrap();

        // Verify update
        let retrieved = db.get_regex_pattern(id).unwrap().unwrap();
        assert_eq!(retrieved.response, "Updated response");

        // Test provider-specific queries
        let patterns = db.get_enabled_regex_patterns_for_provider("test_provider").unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].pattern, pattern.pattern);

        let patterns = db.get_enabled_regex_patterns_for_provider("non_existent_provider").unwrap();
        assert_eq!(patterns.len(), 0);

        // Test provider existence
        assert!(db.provider_has_patterns("test_provider").unwrap());
        assert!(!db.provider_has_patterns("non_existent_provider").unwrap());

        // Delete pattern
        db.delete_regex_pattern(id).unwrap();

        // Verify deletion
        assert!(db.get_regex_pattern(id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_pattern_validation() {
        // Test empty provider name
        let result = RegexPattern::new(
            "".to_string(),
            "pattern".to_string(),
            "response".to_string(),
            None,
            100,
        );
        assert!(result.is_err());

        // Test empty pattern
        let result = RegexPattern::new(
            "provider".to_string(),
            "".to_string(),
            "response".to_string(),
            None,
            100,
        );
        assert!(result.is_err());

        // Test empty response
        let result = RegexPattern::new(
            "provider".to_string(),
            "pattern".to_string(),
            "".to_string(),
            None,
            100,
        );
        assert!(result.is_err());

        // Test valid pattern
        let result = RegexPattern::new(
            "provider".to_string(),
            "pattern".to_string(),
            "response".to_string(),
            None,
            100,
        );
        assert!(result.is_ok());
    }
}

