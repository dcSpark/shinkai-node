use rusqlite::params;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{ShinkaiToolOffering, UsageType};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn set_tool_offering(&self, tool_offering: ShinkaiToolOffering) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO tool_micropayments_requirements (tool_key, usage_type, meta_description)
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
        let mut conn = self.get_connection()?;
        let mut transaction = conn.transaction()?;
        
        // First, nullify references in invoices to prevent constraint violations
        transaction.execute("UPDATE invoices SET shinkai_offering_key = NULL WHERE shinkai_offering_key = ?1", params![tool_key])?;
        
        // Then delete the tool offering
        transaction.execute("DELETE FROM tool_micropayments_requirements WHERE tool_key = ?1", params![tool_key])?;
        
        transaction.commit()?;
        Ok(())
    }

    pub fn get_all_tool_offerings(&self) -> Result<Vec<ShinkaiToolOffering>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT tool_key, usage_type, meta_description FROM tool_micropayments_requirements
                               WHERE tool_key LIKE 'local%'")?;

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
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::{
        shinkai_tool_offering::ToolPrice, x402_types::{Network, PaymentRequirements}
    };
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_set_and_get_tool_offerings() {
        let manager = setup_test_db();
        let tool_offering = ShinkaiToolOffering {
            tool_key: "tool_key".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
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
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
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
    async fn test_upsert_tool_offering() {
        let manager = setup_test_db();
        let tool_offering = ShinkaiToolOffering {
            tool_key: "tool_key".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
            }])),
            meta_description: Some("Original description".to_string()),
        };

        // Insert tool offering
        let result = manager.set_tool_offering(tool_offering.clone());
        assert!(result.is_ok());

        // Verify insertion
        let retrieved_offering = manager.get_tool_offering("tool_key").unwrap();
        assert_eq!(retrieved_offering.meta_description, Some("Original description".to_string()));

        // Update the tool offering with new values
        let updated_tool_offering = ShinkaiToolOffering {
            tool_key: "tool_key".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Updated payment for service".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "2000".to_string(), // 0.002 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 600,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
            }])),
            meta_description: Some("Updated description".to_string()),
        };

        // Update tool offering (should not fail)
        let result = manager.set_tool_offering(updated_tool_offering.clone());
        assert!(result.is_ok());

        // Verify update
        let retrieved_updated_offering = manager.get_tool_offering("tool_key").unwrap();
        assert_eq!(retrieved_updated_offering.meta_description, Some("Updated description".to_string()));
        
        // Verify the usage_type was also updated
        if let UsageType::PerUse(ToolPrice::Payment(ref reqs)) = retrieved_updated_offering.usage_type {
            assert_eq!(reqs[0].max_amount_required, "2000");
            assert_eq!(reqs[0].max_timeout_seconds, 600);
        } else {
            panic!("Expected PerUse usage type");
        }
    }

    #[tokio::test]
    async fn test_get_all_tool_offerings() {
        let manager = setup_test_db();
        let tool_offering1 = ShinkaiToolOffering {
            tool_key: "local:::__localhost_sep_shinkai:::tool_key1".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service 1".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
            }])),
            meta_description: Some("meta_description1".to_string()),
        };

        let tool_offering2 = ShinkaiToolOffering {
            tool_key: "local:::__localhost_sep_shinkai:::tool_key2".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service 2".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
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

    #[tokio::test]
    async fn test_remove_tool_offering_with_invoices() {
        use chrono::Utc;
        use shinkai_message_primitives::schemas::invoices::{Invoice, InvoiceStatusEnum};
        use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
        use shinkai_message_primitives::schemas::shinkai_tool_offering::UsageTypeInquiry;
        use shinkai_message_primitives::schemas::wallet_mixed::PublicAddress;
        use rusqlite::params;
        
        let manager = setup_test_db();
        let tool_key = "test_tool_key_with_invoice";
        
        // First, create a tool offering
        let tool_offering = ShinkaiToolOffering {
            tool_key: tool_key.to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Payment for service".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: "1000".to_string(),
                resource: "https://shinkai.com".to_string(),
                mime_type: "application/json".to_string(),
                pay_to: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                max_timeout_seconds: 300,
                asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                output_schema: Some(serde_json::json!({})),
                extra: Some(serde_json::json!({
                    "decimals": 6,
                    "asset_id": "USDC"
                })),
            }])),
            meta_description: Some("meta_description".to_string()),
        };

        // Insert the tool offering
        let result = manager.set_tool_offering(tool_offering.clone());
        assert!(result.is_ok());

        // Create an invoice that references this tool offering
        let invoice = Invoice {
            invoice_id: "test_invoice_id".to_string(),
            parent_message_id: Some("test_parent_id".to_string()),
            provider_name: ShinkaiName::new("@@provider.shinkai".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@requester.shinkai".to_string()).unwrap(),
            shinkai_offering: tool_offering.clone(),
            expiration_time: Utc::now() + chrono::Duration::hours(12),
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: Network::BaseSepolia,
                address_id: "0x1234567890123456789012345678901234567890".to_string(),
            },
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            request_date_time: Utc::now(),
            invoice_date_time: Utc::now(),
            tool_data: None,
            result_str: None,
            response_date_time: None,
        };

        // Insert the invoice (this creates the foreign key reference)
        let invoice_result = manager.set_invoice(&invoice);
        assert!(invoice_result.is_ok());

        // Now try to remove the tool offering - this should succeed with the new implementation
        let remove_result = manager.remove_tool_offering(tool_key);
        println!("Remove tool offering result: {:?}", remove_result);
        
        // The removal should now succeed because we handle foreign key constraints properly
        assert!(remove_result.is_ok(), "Tool offering removal should succeed");
        
        // Verify that the tool offering was actually removed
        let get_result = manager.get_tool_offering(tool_key);
        assert!(get_result.is_err(), "Tool offering should no longer exist");
        
        // Verify that the invoice still exists but with shinkai_offering_key set to NULL
        // Since get_invoice might not handle NULL foreign keys properly, let's check at the database level
        let conn = manager.get_connection().unwrap();
        let mut stmt = conn.prepare("SELECT invoice_id, shinkai_offering_key FROM invoices WHERE invoice_id = ?1").unwrap();
        let result: Result<(String, Option<String>), _> = stmt.query_row(params!["test_invoice_id"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        });
        
        match result {
            Ok((invoice_id, offering_key)) => {
                println!("Invoice still exists: {}", invoice_id);
                assert_eq!(invoice_id, "test_invoice_id");
                assert!(offering_key.is_none(), "shinkai_offering_key should be NULL after tool offering removal");
                println!("Verified: shinkai_offering_key is NULL as expected");
            },
            Err(err) => {
                panic!("Invoice should still exist after tool offering removal: {:?}", err);
            }
        }
    }
}
