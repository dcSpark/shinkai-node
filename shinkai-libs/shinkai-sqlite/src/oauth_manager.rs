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
    pub response_type: String,
    pub state: String,
    pub code: Option<String>,
    pub app_id: String,
    pub tool_id: String,
    pub tool_key: String,
    pub access_token: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<String>,
    pub refresh_token_enabled: Option<bool>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub refresh_token_expires_at: Option<DateTime<Utc>>,
    pub token_secret: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
    pub pkce_type: Option<String>,
    pub pkce_code_verifier: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata_json: Option<String>,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_url: Option<String>,
    pub version: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
    pub request_token_auth_header: Option<String>,
    pub request_token_content_type: Option<String>,
}

impl SqliteManager {
    pub fn add_oauth_token(&self, token: &OAuthToken) -> Result<i64, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        if token.version.is_empty() {
            return Err(SqliteManagerError::MissingValue("Version is empty".to_string()));
        }
        if token.client_id.clone().unwrap_or_default().is_empty() {
            return Err(SqliteManagerError::MissingValue("Client ID is empty".to_string()));
        }
        if token.client_secret.clone().unwrap_or_default().is_empty() {
            return Err(SqliteManagerError::MissingValue("Client Secret is empty".to_string()));
        }
        if token.redirect_url.clone().unwrap_or_default().is_empty() {
            return Err(SqliteManagerError::MissingValue("Redirect URL is empty".to_string()));
        }
        if token.authorization_url.clone().unwrap_or_default().is_empty() {
            return Err(SqliteManagerError::MissingValue(
                "Authorization URL is empty".to_string(),
            ));
        }
        if token.token_url.clone().unwrap_or_default().is_empty() {
            return Err(SqliteManagerError::MissingValue("Token URL is empty".to_string()));
        }

        tx.execute(
            "INSERT INTO oauth_tokens (
                connection_name,response_type, state, code, app_id, tool_id, tool_key,
                access_token, access_token_expires_at, refresh_token,
                refresh_token_enabled, refresh_token_expires_at, token_secret,
                id_token, scope, pkce_type, pkce_code_verifier,
                expires_at, metadata_json, authorization_url, token_url,
                client_id, client_secret, redirect_url, version, created_at,
                updated_at, request_token_auth_header, request_token_content_type
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,
                      ?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)",
            params![
                token.connection_name,
                token.response_type,
                token.state,
                token.code,
                token.app_id,
                token.tool_id,
                token.tool_key,
                token.access_token,
                token.access_token_expires_at.map(|dt| dt.to_rfc3339()),
                token.refresh_token,
                token.refresh_token_enabled,
                token.refresh_token_expires_at.map(|dt| dt.to_rfc3339()),
                token.token_secret,
                token.id_token,
                token.scope,
                token.pkce_type,
                token.pkce_code_verifier,
                token.expires_at.map(|dt| dt.to_rfc3339()),
                token.metadata_json,
                token.authorization_url,
                token.token_url,
                token.client_id,
                token.client_secret,
                token.redirect_url,
                token.version,
                token.created_at.to_rfc3339(),
                token.updated_at.to_rfc3339(),
                token.request_token_auth_header,
                token.request_token_content_type,
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
                    access_token, access_token_expires_at, refresh_token,
                    refresh_token_enabled, refresh_token_expires_at, token_secret,
                    response_type, id_token, scope, pkce_type, pkce_code_verifier,
                    expires_at, metadata_json, authorization_url, token_url,
                    client_id, client_secret, redirect_url, version, created_at,
                    updated_at, request_token_auth_header, request_token_content_type
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
                access_token_expires_at: row
                    .get::<_, Option<String>>(8)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                refresh_token: row.get(9)?,
                refresh_token_enabled: row.get(10)?,
                refresh_token_expires_at: row
                    .get::<_, Option<String>>(11)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                token_secret: row.get(12)?,
                response_type: row.get(13)?,
                id_token: row.get(14)?,
                scope: row.get(15)?,
                pkce_type: row.get(16)?,
                pkce_code_verifier: row.get(17)?,
                expires_at: row
                    .get::<_, Option<String>>(18)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(19)?,
                authorization_url: row.get(20)?,
                token_url: row.get(21)?,
                client_id: row.get(22)?,
                client_secret: row.get(23)?,
                redirect_url: row.get(24)?,
                version: row.get(25)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(26)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(27)?)
                    .unwrap()
                    .with_timezone(&Utc),
                request_token_auth_header: row.get(28)?,
                request_token_content_type: row.get(29)?,
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
                state = ?1,
                code = ?2,
                app_id = ?3,
                tool_id = ?4,
                access_token = ?5,
                access_token_expires_at = ?6,
                refresh_token = ?7,
                refresh_token_enabled = ?8,
                refresh_token_expires_at = ?9,
                token_secret = ?10,
                response_type = ?11,
                id_token = ?12,
                scope = ?13,
                pkce_type = ?14,
                pkce_code_verifier = ?15,
                expires_at = ?16,
                metadata_json = ?17,
                authorization_url = ?18,
                token_url = ?19,
                client_id = ?20,
                client_secret = ?21,
                redirect_url = ?22,
                version = ?23,
                request_token_auth_header = ?24,
                request_token_content_type = ?25,
                updated_at = ?26
            WHERE connection_name = ?27 and tool_key = ?28",
            params![
                token.state,
                token.code,
                token.app_id,
                token.tool_id,
                token.access_token,
                token.access_token_expires_at.map(|dt| dt.to_rfc3339()),
                token.refresh_token,
                token.refresh_token_enabled,
                token.refresh_token_expires_at.map(|dt| dt.to_rfc3339()),
                token.token_secret,
                token.response_type,
                token.id_token,
                token.scope,
                token.pkce_type,
                token.pkce_code_verifier,
                token.expires_at.map(|dt| dt.to_rfc3339()),
                token.metadata_json,
                token.authorization_url,
                token.token_url,
                token.client_id,
                token.client_secret,
                token.redirect_url,
                token.version,
                token.request_token_auth_header,
                token.request_token_content_type,
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
                    access_token, access_token_expires_at, refresh_token,
                    refresh_token_enabled, refresh_token_expires_at, token_secret,
                    response_type, id_token, scope, pkce_type, pkce_code_verifier,
                    expires_at, metadata_json, authorization_url, token_url,
                    client_id, client_secret, redirect_url, version, created_at,
                    updated_at, request_token_auth_header, request_token_content_type
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
                access_token_expires_at: row
                    .get::<_, Option<String>>(8)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                refresh_token: row.get(9)?,
                refresh_token_enabled: row.get(10)?,
                refresh_token_expires_at: row
                    .get::<_, Option<String>>(11)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                token_secret: row.get(12)?,
                response_type: row.get(13)?,
                id_token: row.get(14)?,
                scope: row.get(15)?,
                pkce_type: row.get(16)?,
                pkce_code_verifier: row.get(17)?,
                expires_at: row
                    .get::<_, Option<String>>(18)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(19)?,
                authorization_url: row.get(20)?,
                token_url: row.get(21)?,
                client_id: row.get(22)?,
                client_secret: row.get(23)?,
                redirect_url: row.get(24)?,
                version: row.get(25)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(26)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(27)?)
                    .unwrap()
                    .with_timezone(&Utc),
                request_token_auth_header: row.get(28)?,
                request_token_content_type: row.get(29)?,
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
                    access_token, access_token_expires_at, refresh_token,
                    refresh_token_enabled, refresh_token_expires_at, token_secret,
                    response_type, id_token, scope, pkce_type, pkce_code_verifier,
                    expires_at, metadata_json, authorization_url, token_url,
                    client_id, client_secret, redirect_url, version, created_at,
                    updated_at, request_token_auth_header, request_token_content_type
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
                access_token_expires_at: row
                    .get::<_, Option<String>>(8)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                refresh_token: row.get(9)?,
                refresh_token_enabled: row.get(10)?,
                refresh_token_expires_at: row
                    .get::<_, Option<String>>(11)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                token_secret: row.get(12)?,
                response_type: row.get(13)?,
                id_token: row.get(14)?,
                scope: row.get(15)?,
                pkce_type: row.get(16)?,
                pkce_code_verifier: row.get(17)?,
                expires_at: row
                    .get::<_, Option<String>>(18)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                metadata_json: row.get(19)?,
                authorization_url: row.get(20)?,
                token_url: row.get(21)?,
                client_id: row.get(22)?,
                client_secret: row.get(23)?,
                redirect_url: row.get(24)?,
                version: row.get(25)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(26)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(27)?)
                    .unwrap()
                    .with_timezone(&Utc),
                request_token_auth_header: row.get(28)?,
                request_token_content_type: row.get(29)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
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
            id: 0,
            connection_name: "test_connection".to_string(),
            state: "test_state".to_string(),
            code: None,
            app_id: "1".to_string(),
            tool_id: "2".to_string(),
            tool_key: "test_tool".to_string(),
            access_token: Some("access_token".to_string()),
            access_token_expires_at: Some(Utc::now()),
            refresh_token: Some("refresh_token".to_string()),
            refresh_token_enabled: Some(true),
            refresh_token_expires_at: Some(Utc::now()),
            token_secret: None,
            response_type: "code".to_string(),
            id_token: None,
            scope: Some("read write".to_string()),
            pkce_type: None,
            pkce_code_verifier: None,
            expires_at: Some(Utc::now()),
            metadata_json: Some(r#"{"key": "value"}"#.to_string()),
            authorization_url: Some("https://example.com/oauth/authorize".to_string()),
            token_url: Some("https://example.com/oauth/token".to_string()),
            client_id: Some("client123".to_string()),
            client_secret: Some("secret456".to_string()),
            redirect_url: Some("https://example.com/callback".to_string()),
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            request_token_auth_header: None,
            request_token_content_type: None,
        }
    }

    #[test]
    fn test_oauth_token_crud_operations() {
        let manager = setup_test_db();

        // Create initial token
        let mut token = create_test_token();
        let token_id = manager.add_oauth_token(&token).unwrap();
        token.id = token_id;

        // Read and verify all fields
        let retrieved_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        // Verify all fields match
        assert_eq!(retrieved_token.id, token.id);
        assert_eq!(retrieved_token.connection_name, token.connection_name);
        assert_eq!(retrieved_token.state, token.state);
        assert_eq!(retrieved_token.code, token.code);
        assert_eq!(retrieved_token.app_id, token.app_id);
        assert_eq!(retrieved_token.tool_id, token.tool_id);
        assert_eq!(retrieved_token.tool_key, token.tool_key);
        assert_eq!(retrieved_token.access_token, token.access_token);
        assert_eq!(
            retrieved_token.access_token_expires_at.map(|dt| dt.timestamp()),
            token.access_token_expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_token.refresh_token, token.refresh_token);
        assert_eq!(retrieved_token.refresh_token_enabled, token.refresh_token_enabled);
        assert_eq!(
            retrieved_token.refresh_token_expires_at.map(|dt| dt.timestamp()),
            token.refresh_token_expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_token.token_secret, token.token_secret);
        assert_eq!(retrieved_token.response_type, token.response_type);
        assert_eq!(retrieved_token.id_token, token.id_token);
        assert_eq!(retrieved_token.scope, token.scope);
        assert_eq!(retrieved_token.pkce_type, token.pkce_type);
        assert_eq!(retrieved_token.pkce_code_verifier, token.pkce_code_verifier);
        assert_eq!(
            retrieved_token.expires_at.map(|dt| dt.timestamp()),
            token.expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_token.metadata_json, token.metadata_json);
        assert_eq!(retrieved_token.authorization_url, token.authorization_url);
        assert_eq!(retrieved_token.token_url, token.token_url);
        assert_eq!(retrieved_token.client_id, token.client_id);
        assert_eq!(retrieved_token.client_secret, token.client_secret);
        assert_eq!(retrieved_token.redirect_url, token.redirect_url);
        assert_eq!(retrieved_token.version, token.version);
        assert_eq!(retrieved_token.created_at.timestamp(), token.created_at.timestamp());
        assert_eq!(retrieved_token.updated_at.timestamp(), token.updated_at.timestamp());

        // Update all fields with new values
        let mut updated_token = token.clone();
        // This field cannot be updated
        // updated_token.connection_name = "updated_connection".to_string();
        updated_token.state = "updated_state".to_string();
        updated_token.code = Some("new_code".to_string());
        updated_token.app_id = "updated_app".to_string();
        updated_token.tool_id = "updated_tool".to_string();
        // This field cannot be updated
        // updated_token.tool_key = "updated_tool_key".to_string();
        updated_token.access_token = Some("new_access_token".to_string());
        updated_token.access_token_expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        updated_token.refresh_token = Some("new_refresh_token".to_string());
        updated_token.refresh_token_enabled = Some(false);
        updated_token.refresh_token_expires_at = Some(Utc::now() + chrono::Duration::hours(2));
        updated_token.token_secret = Some("new_secret".to_string());
        updated_token.response_type = "token".to_string();
        updated_token.id_token = Some("new_id_token".to_string());
        updated_token.scope = Some("updated_scope".to_string());
        updated_token.pkce_type = Some("new_verifier".to_string());
        updated_token.expires_at = Some(Utc::now() + chrono::Duration::hours(3));
        updated_token.metadata_json = Some(r#"{"updated": "value"}"#.to_string());
        updated_token.authorization_url = Some("https://updated.example.com/oauth/authorize".to_string());
        updated_token.token_url = Some("https://updated.example.com/oauth/token".to_string());
        updated_token.client_id = Some("updated_client_id".to_string());
        updated_token.client_secret = Some("updated_client_secret".to_string());
        updated_token.redirect_url = Some("https://updated.example.com/callback".to_string());
        updated_token.version = "2.0.0".to_string();

        // Perform update
        manager.update_oauth_token(&updated_token).unwrap();

        // Read and verify updated fields
        let retrieved_updated_token = manager
            .get_oauth_token(updated_token.connection_name.clone(), updated_token.tool_key.clone())
            .unwrap()
            .unwrap();

        // Verify all updated fields match
        assert_eq!(retrieved_updated_token.connection_name, updated_token.connection_name);
        assert_eq!(retrieved_updated_token.state, updated_token.state);
        assert_eq!(retrieved_updated_token.code, updated_token.code);
        assert_eq!(retrieved_updated_token.app_id, updated_token.app_id);
        assert_eq!(retrieved_updated_token.tool_id, updated_token.tool_id);
        assert_eq!(retrieved_updated_token.tool_key, updated_token.tool_key);
        assert_eq!(retrieved_updated_token.access_token, updated_token.access_token);
        assert_eq!(
            retrieved_updated_token.access_token_expires_at.map(|dt| dt.timestamp()),
            updated_token.access_token_expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_updated_token.refresh_token, updated_token.refresh_token);
        assert_eq!(
            retrieved_updated_token.refresh_token_enabled,
            updated_token.refresh_token_enabled
        );
        assert_eq!(
            retrieved_updated_token
                .refresh_token_expires_at
                .map(|dt| dt.timestamp()),
            updated_token.refresh_token_expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_updated_token.token_secret, updated_token.token_secret);
        assert_eq!(retrieved_updated_token.response_type, updated_token.response_type);
        assert_eq!(retrieved_updated_token.id_token, updated_token.id_token);
        assert_eq!(retrieved_updated_token.scope, updated_token.scope);
        assert_eq!(retrieved_updated_token.pkce_type, updated_token.pkce_type);
        assert_eq!(
            retrieved_updated_token.pkce_code_verifier,
            updated_token.pkce_code_verifier
        );
        assert_eq!(
            retrieved_updated_token.expires_at.map(|dt| dt.timestamp()),
            updated_token.expires_at.map(|dt| dt.timestamp())
        );
        assert_eq!(retrieved_updated_token.metadata_json, updated_token.metadata_json);
        assert_eq!(
            retrieved_updated_token.authorization_url,
            updated_token.authorization_url
        );
        assert_eq!(retrieved_updated_token.token_url, updated_token.token_url);
        assert_eq!(retrieved_updated_token.client_id, updated_token.client_id);
        assert_eq!(retrieved_updated_token.client_secret, updated_token.client_secret);
        assert_eq!(retrieved_updated_token.redirect_url, updated_token.redirect_url);
        assert_eq!(retrieved_updated_token.version, updated_token.version);
    }

    #[test]
    fn test_add_and_get_oauth_token() {
        let manager = setup_test_db();
        let token = create_test_token();
        let connection_name = token.connection_name.clone();
        let tool_key = token.tool_key.clone();
        let state = token.state.clone();

        manager.add_oauth_token(&token).unwrap();
        let retrieved_token = manager.get_oauth_token(connection_name, tool_key).unwrap().unwrap();

        assert_eq!(retrieved_token.state, state);
    }

    #[test]
    fn test_update_oauth_token() {
        let manager = setup_test_db();
        let mut token = create_test_token();

        let _token_id = manager.add_oauth_token(&token).unwrap();
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
        token2.state = format!("{}_2", token2.state);
        token2.connection_name = format!("{}_2", token2.connection_name);
        token2.tool_key = format!("{}_2", token2.tool_key);

        manager.add_oauth_token(&token1).unwrap();
        manager.add_oauth_token(&token2).unwrap();

        let all_tokens = manager.get_all_oauth_tokens().unwrap();
        assert_eq!(all_tokens.len(), 2);
    }

    #[test]
    fn test_expiration_fields() {
        let manager = setup_test_db();
        let now = Utc::now();
        let mut token = create_test_token();

        // Set specific expiration times
        token.access_token_expires_at = Some(now);
        token.refresh_token_expires_at = Some(now);
        token.expires_at = Some(now);

        manager.add_oauth_token(&token).unwrap();
        let retrieved_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        // Verify all expiration fields are preserved
        assert_eq!(
            retrieved_token.access_token_expires_at.map(|dt| dt.timestamp()),
            Some(now.timestamp())
        );
        assert_eq!(
            retrieved_token.refresh_token_expires_at.map(|dt| dt.timestamp()),
            Some(now.timestamp())
        );
        assert_eq!(
            retrieved_token.expires_at.map(|dt| dt.timestamp()),
            Some(now.timestamp())
        );
    }

    #[test]
    fn test_oauth_json_to_token_roundtrip() {
        use crate::oauth_manager::OAuthToken;
        use serde_json::json;
        use shinkai_tools_primitives::tools::tool_config::OAuth;

        let manager = setup_test_db();

        // Start with a JSON configuration
        let oauth_json = json!({
            "github": {
                "scope": "repo,user",
                "enablePkce": true,
                "authorizationUrl": "https://github.com/login/oauth/authorize",
                "tokenUrl": "https://github.com/login/oauth/access_token",
                "clientId": "test_client_id",
                "clientSecret": "test_client_secret",
                "redirectUrl": "https://custom.redirect.com",
                "version": "2.0",
                "responseType": "code",
                "scopes": ["repo", "user"],
                "pkceType": "plain",
                "refreshToken": true
            }
        });

        // Convert JSON to OAuth
        let oauth = OAuth::from_value(&oauth_json).expect("Failed to parse OAuth from JSON");

        // Clone oauth values before any moves occur
        let oauth_auth_url = oauth.authorization_url.clone();
        let oauth_token_url = oauth.token_url.clone();
        let oauth_client_id = oauth.client_id.clone();
        let oauth_client_secret = oauth.client_secret.clone();
        let oauth_redirect_url = oauth.redirect_url.clone();
        let oauth_version = oauth.version.clone();
        let oauth_response_type = oauth.response_type.clone();
        let oauth_scopes = oauth.scopes.clone();
        let oauth_pkce_type = oauth.pkce_type.clone();
        let oauth_refresh_token = oauth.refresh_token.clone();
        let oauth_name = oauth.name.clone();

        // Create OAuthToken from OAuth
        let refresh_token_enabled = if let Some(r) = oauth_refresh_token.clone() {
            Some(r == "true".to_string())
        } else {
            Some(false)
        };
        let token = OAuthToken {
            id: 0,
            connection_name: oauth.name,
            response_type: oauth.response_type,
            state: "test_state".to_string(),
            code: None,
            app_id: "test_app".to_string(),
            tool_id: "github".to_string(),
            tool_key: "github_tool".to_string(),
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
            refresh_token_enabled,
            refresh_token_expires_at: None,
            token_secret: None,
            id_token: None,
            scope: Some(oauth.scopes.join(",")),
            pkce_type: oauth.pkce_type,
            pkce_code_verifier: None,
            expires_at: None,
            metadata_json: None,
            authorization_url: Some(oauth.authorization_url),
            token_url: oauth.token_url,
            client_id: Some(oauth.client_id),
            client_secret: Some(oauth.client_secret),
            redirect_url: Some(oauth.redirect_url),
            version: oauth.version,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            request_token_auth_header: None,
            request_token_content_type: None,
        };

        // Save token to database
        manager.add_oauth_token(&token).unwrap();

        // Retrieve token from database
        let retrieved_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        // Convert back to OAuth and verify
        let restored_oauth = OAuth {
            name: retrieved_token.connection_name,
            pkce_type: retrieved_token.pkce_type,
            authorization_url: retrieved_token.authorization_url.unwrap(),
            token_url: retrieved_token.token_url.clone(),
            client_id: retrieved_token.client_id.unwrap(),
            client_secret: retrieved_token.client_secret.unwrap(),
            redirect_url: retrieved_token.redirect_url.unwrap(),
            version: retrieved_token.version,
            response_type: retrieved_token.response_type,
            scopes: retrieved_token
                .scope
                .unwrap_or_default()
                .split(',')
                .map(String::from)
                .collect(),
            refresh_token: retrieved_token.refresh_token,
            request_token_auth_header: retrieved_token.request_token_auth_header,
            request_token_content_type: retrieved_token.request_token_content_type,
        };

        // Verify the restored OAuth matches the original
        assert_eq!(restored_oauth.name, oauth_name);
        assert_eq!(restored_oauth.authorization_url, oauth_auth_url);
        assert_eq!(restored_oauth.token_url, oauth_token_url);
        assert_eq!(restored_oauth.client_id, oauth_client_id);
        assert_eq!(restored_oauth.client_secret, oauth_client_secret);
        assert_eq!(restored_oauth.redirect_url, oauth_redirect_url);
        assert_eq!(restored_oauth.version, oauth_version);
        assert_eq!(restored_oauth.response_type, oauth_response_type);
        assert_eq!(restored_oauth.scopes, oauth_scopes);
        assert_eq!(restored_oauth.pkce_type, oauth_pkce_type);
        assert_eq!(restored_oauth.refresh_token, oauth_refresh_token);
    }

    #[test]
    fn test_oauth_json_to_token_roundtrip_min() {
        use crate::oauth_manager::OAuthToken;
        use serde_json::json;
        use shinkai_tools_primitives::tools::tool_config::OAuth;

        let manager = setup_test_db();

        // Start with a JSON configuration
        let oauth_json = json!({
            "github": {
                "authorizationUrl": "https://github.com/login/oauth/authorize",
                "tokenUrl": "https://github.com/login/oauth/access_token",
                "clientId": "a",
                "clientSecret": "b",
                "redirectUrl": "https://custom.redirect.com",
                "version": "1.0",
                "responseType": "code",
                "scopes": [],
                "refreshToken": false
            }
        });

        // Convert JSON to OAuth
        let oauth = OAuth::from_value(&oauth_json).expect("Failed to parse OAuth from JSON");

        // Clone oauth values before any moves occur
        let oauth_auth_url = oauth.authorization_url.clone();
        let oauth_token_url = oauth.token_url.clone();
        let oauth_client_id = oauth.client_id.clone();
        let oauth_client_secret = oauth.client_secret.clone();
        let oauth_redirect_url = oauth.redirect_url.clone();
        let oauth_version = oauth.version.clone();
        let oauth_response_type = oauth.response_type.clone();
        let oauth_scopes = oauth.scopes.clone();
        let oauth_pkce_type = oauth.pkce_type.clone();
        let oauth_refresh_token = oauth.refresh_token.clone();
        let oauth_name = oauth.name.clone();

        // Create OAuthToken from OAuth
        let refresh_token_enabled = if let Some(r) = oauth_refresh_token.clone() {
            Some(r == "true".to_string())
        } else {
            Some(false)
        };
        let token = OAuthToken {
            id: 0,
            connection_name: oauth.name,
            response_type: oauth.response_type,
            state: "test_state".to_string(),
            code: None,
            app_id: "test_app".to_string(),
            tool_id: "github".to_string(),
            tool_key: "github_tool".to_string(),
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
            refresh_token_enabled,
            refresh_token_expires_at: None,
            token_secret: None,
            id_token: None,
            scope: Some(oauth.scopes.join(",")),
            pkce_type: oauth.pkce_type,
            pkce_code_verifier: None,
            expires_at: None,
            metadata_json: None,
            authorization_url: Some(oauth.authorization_url),
            token_url: oauth.token_url,
            client_id: Some(oauth.client_id),
            client_secret: Some(oauth.client_secret),
            redirect_url: Some(oauth.redirect_url),
            version: oauth.version,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            request_token_auth_header: None,
            request_token_content_type: None,
        };

        // Save token to database
        manager.add_oauth_token(&token).unwrap();

        // Retrieve token from database
        let retrieved_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        let scopes_data = retrieved_token.scope.unwrap_or_default();
        let scopes = if scopes_data.is_empty() {
            vec![]
        } else {
            scopes_data.split(',').map(String::from).collect()
        };
        // Convert back to OAuth and verify
        let restored_oauth = OAuth {
            name: retrieved_token.connection_name,
            pkce_type: retrieved_token.pkce_type,
            authorization_url: retrieved_token.authorization_url.unwrap(),
            token_url: retrieved_token.token_url.clone(),
            client_id: retrieved_token.client_id.unwrap(),
            client_secret: retrieved_token.client_secret.unwrap(),
            redirect_url: retrieved_token.redirect_url.unwrap(),
            version: retrieved_token.version,
            response_type: retrieved_token.response_type,
            scopes: scopes,
            refresh_token: retrieved_token.refresh_token,
            request_token_auth_header: retrieved_token.request_token_auth_header,
            request_token_content_type: retrieved_token.request_token_content_type,
        };

        // Verify the restored OAuth matches the original
        assert_eq!(restored_oauth.name, oauth_name);
        assert_eq!(restored_oauth.authorization_url, oauth_auth_url);
        assert_eq!(restored_oauth.token_url, oauth_token_url);
        assert_eq!(restored_oauth.client_id, oauth_client_id);
        assert_eq!(restored_oauth.client_secret, oauth_client_secret);
        assert_eq!(restored_oauth.redirect_url, oauth_redirect_url);
        assert_eq!(restored_oauth.version, oauth_version);
        assert_eq!(restored_oauth.response_type, oauth_response_type);
        assert_eq!(restored_oauth.scopes, oauth_scopes);
        assert_eq!(restored_oauth.pkce_type, oauth_pkce_type);
        assert_eq!(restored_oauth.refresh_token, oauth_refresh_token);
    }

    #[test]
    fn test_oauth_token_with_request_headers() {
        let manager = setup_test_db();
        let mut token = create_test_token();

        // Set the new fields
        token.request_token_auth_header = Some("Bearer".to_string());
        token.request_token_content_type = Some("application/json".to_string());

        let token_id = manager.add_oauth_token(&token).unwrap();
        let retrieved_token = manager
            .get_oauth_token(token.connection_name.clone(), token.tool_key.clone())
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_token.request_token_auth_header, Some("Bearer".to_string()));
        assert_eq!(
            retrieved_token.request_token_content_type,
            Some("application/json".to_string())
        );
    }
}
