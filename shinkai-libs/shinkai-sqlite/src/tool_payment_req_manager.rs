use rusqlite::params;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{ShinkaiToolOffering, UsageType};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn set_tool_offering(&self, tool_offering: ShinkaiToolOffering) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO tool_micropayments_requirements (tool_key, usage_type, meta_description)
                VALUES (?1, ?2, ?3)",
        )?;

        let usage_type = serde_json::to_string(&tool_offering.usage_type).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        stmt.execute(params![
            tool_offering.tool_key,
            usage_type,
            tool_offering.meta_description
        ])?;

        Ok(())
    }

    pub fn get_tool_offering(&self, tool_key: &str) -> Result<ShinkaiToolOffering, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT usage_type, meta_description FROM tool_micropayments_requirements WHERE tool_key = ?1")?;

        let tool_offering = stmt
            .query_row(params![tool_key], |row| {
                let usage_type: String = row.get(0)?;
                let usage_type: UsageType = serde_json::from_str(&usage_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

                let meta_description: Option<String> = row.get(1)?;

                Ok(ShinkaiToolOffering {
                    tool_key: tool_key.to_string(),
                    usage_type,
                    meta_description,
                })
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::ToolOfferingNotFound(tool_key.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(tool_offering)
    }

    pub fn remove_tool_offering(&self, tool_key: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM tool_micropayments_requirements WHERE tool_key = ?1")?;

        stmt.execute(params![tool_key])?;

        Ok(())
    }

    pub fn get_all_tool_offerings(&self) -> Result<Vec<ShinkaiToolOffering>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT tool_key, usage_type, meta_description FROM tool_micropayments_requirements")?;

        let tool_offerings = stmt.query_map([], |row| {
            let tool_key: String = row.get(0)?;
            let usage_type: String = row.get(1)?;
            let usage_type: UsageType = serde_json::from_str(&usage_type).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let meta_description: Option<String> = row.get(2)?;

            Ok(ShinkaiToolOffering {
                tool_key,
                usage_type,
                meta_description,
            })
        })?;

        let mut results = Vec::new();
        for tool_offering in tool_offerings {
            results.push(tool_offering?);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::{
        shinkai_tool_offering::{AssetPayment, ToolPrice},
        wallet_mixed::{Asset, NetworkIdentifier},
    };
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

    #[tokio::test]
    async fn test_set_and_get_tool_offerings() {
        let manager = setup_test_db();
        let tool_offering = ShinkaiToolOffering {
            tool_key: "tool_key".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }])),
            meta_description: Some("meta_description".to_string()),
        };

        // Insert tool offering
        let result = manager.set_tool_offering(tool_offering.clone());
        assert!(result.is_ok());

        let tool_offering = manager.get_tool_offering("tool_key").unwrap();
        assert_eq!(tool_offering.tool_key, tool_offering.tool_key);
        assert_eq!(tool_offering.usage_type, tool_offering.usage_type);
        assert_eq!(tool_offering.meta_description, tool_offering.meta_description);
    }

    #[tokio::test]
    async fn test_remove_tool_offerings() {
        let manager = setup_test_db();
        let tool_offering = ShinkaiToolOffering {
            tool_key: "tool_key".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }])),
            meta_description: Some("meta_description".to_string()),
        };

        // Insert tool offering
        let result = manager.set_tool_offering(tool_offering.clone());
        assert!(result.is_ok());

        // Remove tool offering
        let result = manager.remove_tool_offering("tool_key");
        assert!(result.is_ok());

        // Verify that tool offering was removed
        let result = manager.get_tool_offering("tool_key");
        assert!(matches!(result, Err(SqliteManagerError::ToolOfferingNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_all_tool_offerings() {
        let manager = setup_test_db();
        let tool_offering1 = ShinkaiToolOffering {
            tool_key: "tool_key1".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }])),
            meta_description: Some("meta_description1".to_string()),
        };

        let tool_offering2 = ShinkaiToolOffering {
            tool_key: "tool_key2".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }])),
            meta_description: Some("meta_description2".to_string()),
        };

        // Insert tool offerings
        let result = manager.set_tool_offering(tool_offering1.clone());
        assert!(result.is_ok());

        let result = manager.set_tool_offering(tool_offering2.clone());
        assert!(result.is_ok());

        // Get all tool offerings
        let tool_offerings = manager.get_all_tool_offerings().unwrap();
        assert_eq!(tool_offerings.len(), 2);
        assert!(tool_offerings
            .iter()
            .any(|offering| offering.tool_key == tool_offering1.tool_key));
        assert!(tool_offerings
            .iter()
            .any(|offering| offering.tool_key == tool_offering2.tool_key));
    }
}
