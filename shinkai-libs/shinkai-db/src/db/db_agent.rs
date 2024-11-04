use super::{db_main::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

use serde_json::{from_slice, to_vec};
use shinkai_message_primitives::schemas::{llm_providers::agent::Agent, shinkai_name::ShinkaiName};

impl ShinkaiDB {
    pub fn add_agent(&self, agent: Agent, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Construct the database key for the agent
        let agent_id_for_db = Self::db_llm_provider_id(&agent.agent_id, profile)?;

        // Validate the new ShinkaiName
        let agent_name_str = format!(
            "{}/{}/agent/{}",
            profile.node_name,
            profile.profile_name.clone().unwrap_or_default(),
            agent.agent_id
        );
        let _agent_name = ShinkaiName::new(agent_name_str.clone()).map_err(|_| {
            ShinkaiDBError::InvalidIdentityName(format!("Invalid ShinkaiName: {}", agent_name_str))
        })?;

        // Check for collision with llm_provider_id
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let llm_provider_key = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", agent_id_for_db);
        let llm_provider_exists = self.db.get_cf(cf_node_and_users, llm_provider_key.as_bytes())?.is_some();
        if llm_provider_exists {
            return Err(ShinkaiDBError::IdCollision(format!(
                "ID collision detected for agent_id: {}",
                agent.agent_id
            )));
        }

        // Serialize the agent to bytes
        let bytes = to_vec(&agent).unwrap();
        let agent_key = format!("new_agentic_placeholder_values_to_match_prefix_{}", agent.agent_id);

        // Add the agent to the database under NodeAndUsers
        self.db.put_cf(cf_node_and_users, agent_key.as_bytes(), bytes)?;

        Ok(())
    }

    pub fn remove_agent(&self, agent_id: &str) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_key = format!("new_agentic_placeholder_values_to_match_prefix_{}", agent_id);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        // Remove the agent from the database
        self.db.delete_cf(cf_node_and_users, agent_key.as_bytes())?;

        Ok(())
    }

    pub fn get_all_agents(&self) -> Result<Vec<Agent>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let mut result = Vec::new();
        let prefix = b"new_agentic_placeholder_values_to_match_prefix_";

        let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix);
        for item in iter {
            match item {
                Ok((_, value)) => {
                    let agent: Agent = from_slice(value.as_ref()).unwrap();
                    result.push(agent);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(result)
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<Option<Agent>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_key = format!("new_agentic_placeholder_values_to_match_prefix_{}", agent_id);
        let agent_bytes = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?;

        if let Some(bytes) = agent_bytes {
            let agent: Agent = from_slice(&bytes)?;
            Ok(Some(agent))
        } else {
            Ok(None)
        }
    }

    pub fn update_agent(&self, updated_agent: Agent) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_key = format!("new_agentic_placeholder_values_to_match_prefix_{}", updated_agent.agent_id);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        // Serialize the updated agent to bytes
        let bytes = to_vec(&updated_agent).unwrap();

        // Update the agent in the database
        self.db.put_cf(cf_node_and_users, agent_key.as_bytes(), bytes)?;

        Ok(())
    }
}