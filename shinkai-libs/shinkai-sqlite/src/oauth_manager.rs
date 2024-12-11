use crate::SqliteManager;
use crate::SqliteManagerError;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

// Define the OAuth token structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub id: i64,
    pub connection_name: String,
    pub state: String,
    pub code: Option<String>,
    pub app_id: String,
    pub tool_id: String,
    pub tool_key: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_secret: Option<String>,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata_json: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

impl SqliteManager {
    pub fn add_oauth_token(&self, token: &OAuthToken) -> Result<i64, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO oauth_tokens (
                id, connection_name, state, code, app_id, tool_id, tool_key,
                access_token, refresh_token, token_secret, token_type,
                id_token, scope, expires_at, metadata_json, created_at, updated_at
            ) VALUES (NULL,?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                token.connection_name,
                token.state,
                token.code,
                token.app_id,
                token.tool_id,
                token.tool_key,
                token.access_token,
                token.refresh_token,
                token.token_secret,
                token.token_type,
                token.id_token,
                token.scope,
                token.expires_at.map(|dt| dt.to_rfc3339()),
                token.metadata_json,
                token.created_at.to_rfc3339(),
                token.updated_at.to_rfc3339(),
            ],
        )?;

        let token_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(token_id)
    }

    pub fn remove_oauth_token(&self, token_id: i64) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM oauth_tokens WHERE id = ?1", params![token_id])?;
        Ok(())
    }

    pub fn get_oauth_token(
        &self,
        connection_name: String,
        tool_key: String,
    ) -> Result<Option<OAuthToken>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, connection_name, state, code, app_id, tool_id, tool_key,
                    access_token, refresh_token, token_secret, token_type,
                    id_token, scope, expires_at, metadata_json, created_at, updated_at
             FROM oauth_tokens WHERE connection_name = ?1 and tool_key = ?2",
        )?;

        let mut rows = stmt.query(params![connection_name, tool_key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(OAuthToken {
                id: row.get(0)?,
                connection_name: row.get(1)?,
                state: row.get(2)?,
                code: row.get(3)?,
                app_id: row.get(4)?,
                tool_id: row.get(5)?,
                tool_key: row.get(6)?,
                access_token: row.get(7)?,
                refresh_token: row.get(8)?,
                token_secret: row.get(9)?,
                token_type: row.get(10)?,
                id_token: row.get(11)?,
                scope: row.get(12)?,
                expires_at: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(14)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(15)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .unwrap()
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_oauth_token(&self, token: &OAuthToken) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "UPDATE oauth_tokens SET 
                connection_name = ?1,
                state = ?2,
                code = ?3,
                app_id = ?4,
                tool_id = ?5,
                tool_key = ?6,
                access_token = ?7,
                refresh_token = ?8,
                token_secret = ?9,
                token_type = ?10,
                id_token = ?11,
                scope = ?12,
                expires_at = ?13,
                metadata_json = ?14,
                updated_at = ?15
            WHERE connection_name = ?16 and tool_key = ?17",
            params![
                token.connection_name,
                token.state,
                token.code,
                token.app_id,
                token.tool_id,
                token.tool_key,
                token.access_token,
                token.refresh_token,
                token.token_secret,
                token.token_type,
                token.id_token,
                token.scope,
                token.expires_at.map(|dt| dt.to_rfc3339()),
                token.metadata_json,
                Utc::now().to_rfc3339(),
                token.connection_name,
                token.tool_key,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_all_oauth_tokens(&self) -> Result<Vec<OAuthToken>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, connection_name, state, code, app_id, tool_id, tool_key,
                    access_token, refresh_token, token_secret, token_type,
                    id_token, scope, expires_at, metadata_json, created_at, updated_at
             FROM oauth_tokens",
        )?;

        let token_iter = stmt.query_map([], |row| {
            Ok(OAuthToken {
                id: row.get(0)?,
                connection_name: row.get(1)?,
                state: row.get(2)?,
                code: row.get(3)?,
                app_id: row.get(4)?,
                tool_id: row.get(5)?,
                tool_key: row.get(6)?,
                access_token: row.get(7)?,
                refresh_token: row.get(8)?,
                token_secret: row.get(9)?,
                token_type: row.get(10)?,
                id_token: row.get(11)?,
                scope: row.get(12)?,
                expires_at: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(14)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(15)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        token_iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteManagerError::DatabaseError)
    }

    pub fn get_oauth_token_by_state(&self, state: &str) -> Result<Option<OAuthToken>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, connection_name, state, code, app_id, tool_id, tool_key,
                    access_token, refresh_token, token_secret, token_type,
                    id_token, scope, expires_at, metadata_json, created_at, updated_at
             FROM oauth_tokens WHERE state = ?1",
        )?;

        let mut rows = stmt.query(params![state])?;

        if let Some(row) = rows.next()? {
            Ok(Some(OAuthToken {
                id: row.get(0)?,
                connection_name: row.get(1)?,
                state: row.get(2)?,
                code: row.get(3)?,
                app_id: row.get(4)?,
                tool_id: row.get(5)?,
                tool_key: row.get(6)?,
                access_token: row.get(7)?,
                refresh_token: row.get(8)?,
                token_secret: row.get(9)?,
                token_type: row.get(10)?,
                id_token: row.get(11)?,
                scope: row.get(12)?,
                expires_at: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(14)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(15)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .unwrap()
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_token() -> OAuthToken {
        OAuthToken {
            id: 0, // Will be set by the database
            connection_name: "test_connection".to_string(),
            state: "test_state".to_string(),
            code: None,
            app_id: "1".to_string(),
            tool_id: "2".to_string(),
            tool_key: "test_tool".to_string(),
            access_token: Some("access_token".to_string()),
            refresh_token: Some("refresh_token".to_string()),
            token_secret: None,
            token_type: Some("Bearer".to_string()),
            id_token: None,
            scope: Some("read write".to_string()),
            expires_at: Some(Utc::now()),
            metadata_json: Some(r#"{"key": "value"}"#.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_add_and_get_oauth_token() {
        let manager = setup_test_db();
        let token = create_test_token();
        let connection_name = token.connection_name.clone();
        let tool_key = token.tool_key.clone();
        let state = token.state.clone();

        // let token_id = manager.add_oauth_token(&token).unwrap();
        let retrieved_token = manager.get_oauth_token(connection_name, tool_key).unwrap().unwrap();

        assert_eq!(retrieved_token.state, state);
    }

    #[test]
    fn test_update_oauth_token() {
        let manager = setup_test_db();
        let mut token = create_test_token();

        // let token_id = manager.add_oauth_token(&token).unwrap();
        token.access_token = Some("new_access_token".to_string());

        manager.update_oauth_token(&token).unwrap();
        let updated_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        assert_eq!(updated_token.access_token, Some("new_access_token".to_string()));
    }

    #[test]
    fn test_remove_oauth_token() {
        let manager = setup_test_db();
        let token = create_test_token();
        let connection_name = token.connection_name.clone();
        let tool_key = token.tool_key.clone();

        let token_id = manager.add_oauth_token(&token).unwrap();
        manager.remove_oauth_token(token_id).unwrap();

        assert!(manager.get_oauth_token(connection_name, tool_key).unwrap().is_none());
    }

    #[test]
    fn test_get_oauth_token_by_state() {
        let manager = setup_test_db();
        let token = create_test_token();
        let state = token.state.clone();

        manager.add_oauth_token(&token).unwrap();
        let retrieved_token = manager.get_oauth_token_by_state(&state).unwrap().unwrap();

        assert_eq!(retrieved_token.state, state);
    }

    #[test]
    fn test_get_all_oauth_tokens() {
        let manager = setup_test_db();
        let token1 = create_test_token();
        let mut token2 = create_test_token();
        token2.state = "test_state_2".to_string();

        manager.add_oauth_token(&token1).unwrap();
        manager.add_oauth_token(&token2).unwrap();

        let all_tokens = manager.get_all_oauth_tokens().unwrap();
        assert_eq!(all_tokens.len(), 2);
    }
}
