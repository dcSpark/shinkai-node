use shinkai_message_primitives::schemas::invoices::{Invoice, InvoiceRequestNetworkError};

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    pub fn set_invoice(&self, invoice: &Invoice) -> Result<(), ShinkaiDBError> {
        // TODO: find a way that we can store the invoice with sorting capabilities
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_abcdefg_prefix_{}", invoice.invoice_id);
        let invoice_bytes = serde_json::to_vec(&invoice)
            .map_err(|e| ShinkaiDBError::SomeError(format!("Failed to serialize invoice: {:?}", e)))?;
        self.db.put_cf(cf_node, prefix.as_bytes(), invoice_bytes)?;
        Ok(())
    }

    // Function to read an Invoice from the database
    pub fn get_invoice(&self, invoice_id: &str) -> Result<Invoice, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_abcdefg_prefix_{}", invoice_id);
        let invoice_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to retrieve invoice".to_string()))?
            .ok_or(ShinkaiDBError::SomeError("No invoice found".to_string()))?;
        let invoice: Invoice = serde_json::from_slice(&invoice_bytes)
            .map_err(|e| ShinkaiDBError::SomeError(format!("Failed to deserialize invoice: {:?}", e)))?;
        Ok(invoice)
    }

    // Function to get all Invoices
    pub fn get_all_invoices(&self) -> Result<Vec<Invoice>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = "tool_micropayments_tool_invoice_abcdefg_prefix_".as_bytes();
        let iter = self.db.prefix_iterator_cf(cf_node, prefix);
        let mut results = Vec::new();

        for item in iter {
            let (_key, value) = match item {
                Ok((key, value)) => (key, value),
                Err(_) => return Err(ShinkaiDBError::SomeError("Iterator error".to_string())),
            };

            let invoice: Invoice = serde_json::from_slice(&value).map_err(|e| {
                ShinkaiDBError::SomeError(format!("Failed converting JSON bytes back to invoice: {:?}", e))
            })?;

            results.push(invoice);
        }

        Ok(results)
    }

    // Function to remove an Invoice from the database
    pub fn remove_invoice(&self, invoice_id: &str) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_abcdefg_prefix_{}", invoice_id);
        self.db
            .delete_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to remove invoice".to_string()))?;
        Ok(())
    }

    // Network errors
    pub fn set_invoice_network_error(&self, error: &InvoiceRequestNetworkError) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_network_errors_{}", error.invoice_id);
        let error_bytes = serde_json::to_vec(&error)
            .map_err(|e| ShinkaiDBError::SomeError(format!("Failed to serialize invoice network error: {:?}", e)))?;
        self.db.put_cf(cf_node, prefix.as_bytes(), error_bytes)?;
        Ok(())
    }

    pub fn get_invoice_network_error(&self, invoice_id: &str) -> Result<InvoiceRequestNetworkError, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_network_errors_{}", invoice_id);
        let error_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to retrieve invoice network error".to_string()))?
            .ok_or(ShinkaiDBError::SomeError("No invoice network error found".to_string()))?;
        let error: InvoiceRequestNetworkError = serde_json::from_slice(&error_bytes)
            .map_err(|e| ShinkaiDBError::SomeError(format!("Failed to deserialize invoice network error: {:?}", e)))?;
        Ok(error)
    }

    pub fn get_all_invoice_network_errors(&self) -> Result<Vec<InvoiceRequestNetworkError>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = "tool_micropayments_tool_invoice_network_errors_".as_bytes();
        let iter = self.db.prefix_iterator_cf(cf_node, prefix);
        let mut results = Vec::new();

        for item in iter {
            let (_key, value) = match item {
                Ok((key, value)) => (key, value),
                Err(_) => return Err(ShinkaiDBError::SomeError("Iterator error".to_string())),
            };

            let error: InvoiceRequestNetworkError = serde_json::from_slice(&value).map_err(|e| {
                ShinkaiDBError::SomeError(format!("Failed converting JSON bytes back to invoice network error: {:?}", e))
            })?;

            results.push(error);
        }

        Ok(results)
    }

    pub fn remove_invoice_network_error(&self, invoice_id: &str) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_tool_invoice_network_errors_{}", invoice_id);
        self.db
            .delete_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to remove invoice network error".to_string()))?;
        Ok(())
    }
}
