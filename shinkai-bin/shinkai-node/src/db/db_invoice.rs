use crate::network::agent_payments_manager::external_agent_payments_manager::Invoice;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    pub fn set_invoice(&self, invoice: &Invoice) -> Result<(), ShinkaiDBError> {
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
            let (key, value) = match item {
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
}
