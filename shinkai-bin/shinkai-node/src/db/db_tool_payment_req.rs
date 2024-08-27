use crate::network::agent_payments_manager::shinkai_tool_offering::ShinkaiToolOffering;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    pub fn set_tool_offering(
        &self,
        tool_offering: ShinkaiToolOffering,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_requirements_abcdefg_prefix_{}", tool_offering.tool_key_name);
        let offering_bytes = serde_json::to_vec(&tool_offering).map_err(|e| {
            ShinkaiDBError::SomeError(format!("Failed to serialize tool offering: {:?}", e))
        })?;
        self.db.put_cf(cf_node, prefix.as_bytes(), offering_bytes)?;
        Ok(())
    }

    pub fn get_tool_offering(
        &self,
        tool_key_name: &str,
    ) -> Result<ShinkaiToolOffering, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_requirements_abcdefg_prefix_{}", tool_key_name);
        let offering_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to retrieve tool offering".to_string()))?
            .ok_or(ShinkaiDBError::SomeError("No tool offering found".to_string()))?;
        let tool_offering: ShinkaiToolOffering = serde_json::from_slice(&offering_bytes).map_err(|e| {
            ShinkaiDBError::SomeError(format!("Failed to deserialize tool offering: {:?}", e))
        })?;
        Ok(tool_offering)
    }

    pub fn remove_tool_offering(&self, tool_key_name: &str) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("tool_micropayments_requirements_abcdefg_prefix_{}", tool_key_name);
        self.db
            .delete_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to remove tool offering".to_string()))?;
        Ok(())
    }

    pub fn get_all_tool_offerings(&self) -> Result<Vec<ShinkaiToolOffering>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = "tool_micropayments_requirements_abcdefg_prefix_".as_bytes();
        let iter = self.db.prefix_iterator_cf(cf_node, prefix);
        let mut results = Vec::new();

        for item in iter {
            let (key, value) = match item {
                Ok((key, value)) => (key, value),
                Err(_) => return Err(ShinkaiDBError::SomeError("Iterator error".to_string())),
            };

            let tool_offering: ShinkaiToolOffering = serde_json::from_slice(&value).map_err(|e| {
                ShinkaiDBError::SomeError(format!("Failed converting JSON bytes back to tool offering: {:?}", e))
            })?;

            results.push(tool_offering);
        }

        Ok(results)
    }

    // TODO: add something to write an invoice - also we should be able to sort them by date (starting from the most recent)
    // TODO: add something to read an invoice
    // TODO: add something to get X invoices starting from Y (low priority)
}