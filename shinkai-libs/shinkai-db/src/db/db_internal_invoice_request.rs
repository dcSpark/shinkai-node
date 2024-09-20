use shinkai_message_primitives::schemas::invoices::InternalInvoiceRequest;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    // Function to write an InternalInvoiceRequest to the database
    pub fn set_internal_invoice_request(
        &self,
        internal_invoice_request: &InternalInvoiceRequest,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!(
            "tool_micropayments_invoicerequest_abcde_prefix_{}",
            internal_invoice_request.unique_id
        );
        let request_bytes = serde_json::to_vec(&internal_invoice_request)
            .map_err(|e| ShinkaiDBError::SomeError(format!("Failed to serialize internal invoice request: {:?}", e)))?;
        self.db.put_cf(cf_node, prefix.as_bytes(), request_bytes)?;
        Ok(())
    }

    // Function to read an InternalInvoiceRequest from the database
    pub fn get_internal_invoice_request(&self, unique_id: &str) -> Result<InternalInvoiceRequest, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_invoicerequest_abcde_prefix_{}", unique_id);
        let request_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to retrieve internal invoice request".to_string()))?
            .ok_or(ShinkaiDBError::SomeError(
                "No internal invoice request found".to_string(),
            ))?;
        let internal_invoice_request: InternalInvoiceRequest = serde_json::from_slice(&request_bytes).map_err(|e| {
            ShinkaiDBError::SomeError(format!("Failed to deserialize internal invoice request: {:?}", e))
        })?;
        Ok(internal_invoice_request)
    }

    // Function to get all InternalInvoiceRequests
    pub fn get_all_internal_invoice_requests(&self) -> Result<Vec<InternalInvoiceRequest>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = "tool_micropayments_invoicerequest_abcde_prefix_".as_bytes();
        let iter = self.db.prefix_iterator_cf(cf_node, prefix);
        let mut results = Vec::new();

        for item in iter {
            let (_key, value) = match item {
                Ok((key, value)) => (key, value),
                Err(_) => return Err(ShinkaiDBError::SomeError("Iterator error".to_string())),
            };

            let internal_invoice_request: InternalInvoiceRequest = serde_json::from_slice(&value).map_err(|e| {
                ShinkaiDBError::SomeError(format!(
                    "Failed converting JSON bytes back to internal invoice request: {:?}",
                    e
                ))
            })?;

            results.push(internal_invoice_request);
        }

        Ok(results)
    }

    // Function to remove an InternalInvoiceRequest from the database
    pub fn remove_internal_invoice_request(&self, unique_id: &str) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_invoicerequest_abcde_prefix_{}", unique_id);
        self.db
            .delete_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to remove internal invoice request".to_string()))?;
        Ok(())
    }
}
