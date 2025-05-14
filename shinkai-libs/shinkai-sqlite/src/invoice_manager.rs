use rusqlite::params;
use shinkai_message_primitives::schemas::{
    invoices::{Invoice, InvoiceRequestNetworkError},
    shinkai_name::ShinkaiName,
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn set_invoice(&self, invoice: &Invoice) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO invoices (
                invoice_id,
                provider_name,
                requester_name,
                usage_type_inquiry,
                shinkai_offering_key,
                request_date_time,
                invoice_date_time,
                expiration_time,
                status,
                payment,
                address, 
                tool_data,
                response_date_time,
                result_str
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        // Add tool offering if it does not exist
        match self.get_tool_offering(&invoice.shinkai_offering.tool_key) {
            Err(SqliteManagerError::ToolOfferingNotFound(_)) => {
                self.set_tool_offering(invoice.shinkai_offering.clone())?
            }
            Err(e) => return Err(e),
            Ok(_) => {}
        }

        stmt.execute(params![
            invoice.invoice_id,
            invoice.provider_name.full_name,
            invoice.requester_name.full_name,
            serde_json::to_string(&invoice.usage_type_inquiry).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            invoice.shinkai_offering.tool_key,
            invoice.request_date_time.to_rfc3339(),
            invoice.invoice_date_time.to_rfc3339(),
            invoice.expiration_time.to_rfc3339(),
            serde_json::to_string(&invoice.status).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_string(&invoice.payment).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_string(&invoice.address).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_vec(&invoice.tool_data).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            invoice.response_date_time.map(|dt| dt.to_rfc3339()),
            invoice.result_str,
        ])?;

        Ok(())
    }

    pub fn get_invoice(&self, invoice_id: &str) -> Result<Invoice, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM invoices WHERE invoice_id = ?1")?;

        let invoice = stmt
            .query_row(params![invoice_id], |row| {
                let provider_name: String = row.get(1)?;
                let requester_name: String = row.get(2)?;
                let usage_type_inquiry: String = row.get(3)?;
                let shinkai_offering_key: String = row.get(4)?;
                let request_date_time: String = row.get(5)?;
                let invoice_date_time: String = row.get(6)?;
                let expiration_time: String = row.get(7)?;
                let status: String = row.get(8)?;
                let payment: String = row.get(9)?;
                let address: String = row.get(10)?;
                let tool_data: Vec<u8> = row.get(11)?;
                let response_date_time: Option<String> = row.get(12)?;

                Ok(Invoice {
                    invoice_id: row.get(0)?,
                    provider_name: ShinkaiName::new(provider_name).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    requester_name: ShinkaiName::new(requester_name).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    usage_type_inquiry: serde_json::from_str(&usage_type_inquiry).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    shinkai_offering: self
                        .get_tool_offering(&shinkai_offering_key)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    request_date_time: request_date_time
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    invoice_date_time: invoice_date_time
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    expiration_time: expiration_time.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    status: serde_json::from_str(&status).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    payment: serde_json::from_str(&payment).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    address: serde_json::from_str(&address).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    tool_data: serde_json::from_slice(&tool_data).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    response_date_time: match response_date_time {
                        Some(dt) => Some(dt.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?),
                        None => None,
                    },
                    result_str: row.get(13)?,
                })
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::InvoiceNotFound(invoice_id.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(invoice)
    }

    pub fn get_all_invoices(&self) -> Result<Vec<Invoice>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM invoices")?;

        let invoices = stmt
            .query_map([], |row| {
                let provider_name: String = row.get(1)?;
                let requester_name: String = row.get(2)?;
                let usage_type_inquiry: String = row.get(3)?;
                let shinkai_offering_key: String = row.get(4)?;
                let request_date_time: String = row.get(5)?;
                let invoice_date_time: String = row.get(6)?;
                let expiration_time: String = row.get(7)?;
                let status: String = row.get(8)?;
                let payment: String = row.get(9)?;
                let address: String = row.get(10)?;
                let tool_data: Vec<u8> = row.get(11)?;
                let response_date_time: Option<String> = row.get(12)?;

                Ok(Invoice {
                    invoice_id: row.get(0)?,
                    provider_name: ShinkaiName::new(provider_name).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    requester_name: ShinkaiName::new(requester_name).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    usage_type_inquiry: serde_json::from_str(&usage_type_inquiry).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    shinkai_offering: self
                        .get_tool_offering(&shinkai_offering_key)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    request_date_time: request_date_time
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    invoice_date_time: invoice_date_time
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    expiration_time: expiration_time.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    status: serde_json::from_str(&status).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    payment: serde_json::from_str(&payment).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    address: serde_json::from_str(&address).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    tool_data: serde_json::from_slice(&tool_data).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    response_date_time: match response_date_time {
                        Some(dt) => Some(dt.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?),
                        None => None,
                    },
                    result_str: row.get(13)?,
                })
            })
            .map_err(SqliteManagerError::DatabaseError)?;

        let mut results = Vec::new();
        for invoice in invoices {
            results.push(invoice?);
        }

        Ok(results)
    }

    pub fn remove_invoice(&self, invoice_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM invoices WHERE invoice_id = ?1")?;
        stmt.execute(params![invoice_id])?;

        Ok(())
    }

    // Network errors

    pub fn set_invoice_network_error(&self, error: &InvoiceRequestNetworkError) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO invoice_network_errors (
                invoice_id,
                provider_name,
                requester_name,
                request_date_time,
                response_date_time,
                user_error_message,
                error_message
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        stmt.execute(params![
            error.invoice_id,
            error.provider_name.full_name,
            error.requester_name.full_name,
            error.request_date_time.to_rfc3339(),
            error.response_date_time.to_rfc3339(),
            error.user_error_message,
            error.error_message,
        ])?;

        Ok(())
    }

    pub fn get_invoice_network_error(
        &self,
        invoice_id: &str,
    ) -> Result<InvoiceRequestNetworkError, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM invoice_network_errors WHERE invoice_id = ?1")?;

        let error = stmt
            .query_row(params![invoice_id], |row| {
                Ok(InvoiceRequestNetworkError {
                    invoice_id: row.get(0)?,
                    provider_name: ShinkaiName::new(row.get(1)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    requester_name: ShinkaiName::new(row.get(2)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    request_date_time: row
                        .get::<_, String>(3)?
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    response_date_time: row
                        .get::<_, String>(4)?
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    user_error_message: row.get(5)?,
                    error_message: row.get(6)?,
                })
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::InvoiceNetworkErrorNotFound(invoice_id.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(error)
    }

    pub fn get_all_invoice_network_errors(&self) -> Result<Vec<InvoiceRequestNetworkError>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM invoice_network_errors")?;

        let errors = stmt
            .query_map([], |row| {
                Ok(InvoiceRequestNetworkError {
                    invoice_id: row.get(0)?,
                    provider_name: ShinkaiName::new(row.get(1)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    requester_name: ShinkaiName::new(row.get(2)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?,
                    request_date_time: row
                        .get::<_, String>(3)?
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    response_date_time: row
                        .get::<_, String>(4)?
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    user_error_message: row.get(5)?,
                    error_message: row.get(6)?,
                })
            })
            .map_err(SqliteManagerError::DatabaseError)?;

        let mut results = Vec::new();
        for error in errors {
            results.push(error?);
        }

        Ok(results)
    }

    pub fn remove_invoice_network_error(&self, invoice_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM invoice_network_errors WHERE invoice_id = ?1")?;
        stmt.execute(params![invoice_id])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::{
        invoices::InvoiceStatusEnum,
        shinkai_name::ShinkaiName,
        shinkai_tool_offering::{ShinkaiToolOffering, ToolPrice, UsageType, UsageTypeInquiry},
        wallet_mixed::{NetworkIdentifier, PublicAddress},
    };
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
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

    #[test]
    fn test_set_invoice() {
        let db = setup_test_db();
        let invoice = Invoice {
            invoice_id: "invoice_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: "tool_key".to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Free),
                meta_description: None,
            },
            request_date_time: chrono::Utc::now(),
            invoice_date_time: chrono::Utc::now(),
            expiration_time: chrono::Utc::now(),
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: NetworkIdentifier::BaseSepolia,
                address_id: "address_id".to_string(),
            },
            tool_data: None,
            response_date_time: Some(chrono::Utc::now()),
            result_str: Some("result_str".to_string()),
        };

        db.set_invoice(&invoice).unwrap();
        let invoice_from_db = db.get_invoice("invoice_id").unwrap();
        assert_eq!(invoice, invoice_from_db);
    }

    #[test]
    fn test_get_all_invoices() {
        let db = setup_test_db();
        let invoice1 = Invoice {
            invoice_id: "invoice_id1".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: "tool_key".to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Free),
                meta_description: None,
            },
            request_date_time: chrono::Utc::now(),
            invoice_date_time: chrono::Utc::now(),
            expiration_time: chrono::Utc::now(),
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: NetworkIdentifier::BaseSepolia,
                address_id: "address_id".to_string(),
            },
            tool_data: None,
            response_date_time: Some(chrono::Utc::now()),
            result_str: Some("result_str".to_string()),
        };

        let invoice2 = Invoice {
            invoice_id: "invoice_id2".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: "tool_key".to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Free),
                meta_description: None,
            },
            request_date_time: chrono::Utc::now(),
            invoice_date_time: chrono::Utc::now(),
            expiration_time: chrono::Utc::now(),
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: NetworkIdentifier::BaseSepolia,
                address_id: "address_id".to_string(),
            },
            tool_data: None,
            response_date_time: Some(chrono::Utc::now()),
            result_str: Some("result_str".to_string()),
        };

        db.set_invoice(&invoice1).unwrap();
        db.set_invoice(&invoice2).unwrap();

        let invoices = db.get_all_invoices().unwrap();
        assert_eq!(invoices.len(), 2);
        assert!(invoices.contains(&invoice1));
        assert!(invoices.contains(&invoice2));
    }

    #[test]
    fn test_remove_invoice() {
        let db = setup_test_db();
        let invoice = Invoice {
            invoice_id: "invoice_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: "tool_key".to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Free),
                meta_description: None,
            },
            request_date_time: chrono::Utc::now(),
            invoice_date_time: chrono::Utc::now(),
            expiration_time: chrono::Utc::now(),
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: NetworkIdentifier::BaseSepolia,
                address_id: "address_id".to_string(),
            },
            tool_data: None,
            response_date_time: Some(chrono::Utc::now()),
            result_str: Some("result_str".to_string()),
        };

        db.set_invoice(&invoice).unwrap();
        db.remove_invoice("invoice_id").unwrap();
        let invoices = db.get_all_invoices().unwrap();
        assert!(invoices.is_empty());
    }

    #[test]
    fn test_set_invoice_network_error() {
        let db = setup_test_db();
        let error = InvoiceRequestNetworkError {
            invoice_id: "invoice_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            request_date_time: chrono::Utc::now(),
            response_date_time: chrono::Utc::now(),
            user_error_message: Some("user_error_message".to_string()),
            error_message: "error_message".to_string(),
        };

        db.set_invoice_network_error(&error).unwrap();
        let error_from_db = db.get_invoice_network_error("invoice_id").unwrap();
        assert_eq!(error, error_from_db);
    }

    #[test]
    fn test_get_all_invoice_network_errors() {
        let db = setup_test_db();
        let error1 = InvoiceRequestNetworkError {
            invoice_id: "invoice_id1".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            request_date_time: chrono::Utc::now(),
            response_date_time: chrono::Utc::now(),
            user_error_message: Some("user_error_message".to_string()),
            error_message: "error_message".to_string(),
        };

        let error2 = InvoiceRequestNetworkError {
            invoice_id: "invoice_id2".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            request_date_time: chrono::Utc::now(),
            response_date_time: chrono::Utc::now(),
            user_error_message: Some("user_error_message".to_string()),
            error_message: "error_message".to_string(),
        };

        db.set_invoice_network_error(&error1).unwrap();
        db.set_invoice_network_error(&error2).unwrap();

        let errors = db.get_all_invoice_network_errors().unwrap();
        assert_eq!(errors.len(), 2);
        assert!(errors.contains(&error1));
        assert!(errors.contains(&error2));
    }

    #[test]
    fn test_remove_invoice_network_error() {
        let db = setup_test_db();
        let error = InvoiceRequestNetworkError {
            invoice_id: "invoice_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            request_date_time: chrono::Utc::now(),
            response_date_time: chrono::Utc::now(),
            user_error_message: Some("user_error_message".to_string()),
            error_message: "error_message".to_string(),
        };

        db.set_invoice_network_error(&error).unwrap();
        db.remove_invoice_network_error("invoice_id").unwrap();
        let errors = db.get_all_invoice_network_errors().unwrap();
        assert!(errors.is_empty());
    }
}
