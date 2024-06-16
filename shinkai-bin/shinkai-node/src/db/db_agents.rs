use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

use serde_json::{from_slice, to_vec};
use shinkai_message_primitives::schemas::{llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::ShinkaiName};

impl ShinkaiDB {
    /// Returns the the first half of the blake3 hash of the agent id value
    pub fn agent_id_to_hash(agent_id: &str) -> String {
        let full_hash = blake3::hash(agent_id.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    // Fetches all agents from the Agents topic
    pub fn get_all_agents(&self) -> Result<Vec<SerializedLLMProvider>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let mut result = Vec::new();
        let prefix = b"agent_placeholder_value_to_match_prefix_abcdef_";

        let iter = self.db.prefix_iterator_cf(cf, prefix);
        for item in iter {
            match item {
                Ok((_, value)) => {
                    let agent: SerializedLLMProvider = from_slice(value.as_ref()).unwrap();
                    result.push(agent);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(result)
    }

    pub fn db_agent_id(agent_id: &str, profile: &ShinkaiName) -> Result<String, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .clone()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile.full_name.to_string()))?;

        Ok(format!("{}:::{}", agent_id, profile_name))
    }

    pub fn add_agent(&self, agent: SerializedLLMProvider, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Serialize the agent to bytes
        let bytes = to_vec(&agent).unwrap();

        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let agent_id_for_db = Self::db_agent_id(&agent.id, profile)?;
        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);

        // Create a new RocksDB WriteBatch
        let mut batch = rocksdb::WriteBatch::default();

        // Prefix the agent ID and write it to the NodeAndUsers topic
        let agent_key = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", &agent_id_for_db);
        batch.put_cf(cf_node_and_users, agent_key.as_bytes(), &bytes);

        let profile_key = format!(
            "agent_{}_profile_{}",
            &agent_id_for_db_hash,
            profile.get_profile_name_string().unwrap_or_default()
        );
        batch.put_cf(cf_node_and_users, profile_key.as_bytes(), []);

        // Additionally, for each allowed message sender and toolkit permission,
        // you can store them with a specific prefix to indicate their relationship to the agent.
        for profile in &agent.allowed_message_senders {
            let profile_key = format!("agent_{}_profile_{}", &agent_id_for_db_hash, profile);
            batch.put_cf(cf_node_and_users, profile_key.as_bytes(), []);
        }
        for toolkit in &agent.toolkit_permissions {
            let toolkit_key = format!("agent_{}_toolkit_{}", &agent_id_for_db_hash, toolkit);
            batch.put_cf(cf_node_and_users, toolkit_key.as_bytes(), []);
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    /// Updates an existing agent in the database.
    pub fn update_agent(&self, updated_agent: SerializedLLMProvider, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Serialize the updated agent to bytes
        let bytes = to_vec(&updated_agent).unwrap();

        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Construct the database key for the agent
        let agent_id_for_db = Self::db_agent_id(&updated_agent.id, profile)?;
        let agent_key = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", &agent_id_for_db);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        // Update the agent in the database
        self.db.put_cf(cf_node_and_users, agent_key.as_bytes(), bytes)?;

        Ok(())
    }

    pub fn remove_agent(&self, agent_id: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Get cf handle for NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
    
        // Construct the key for the specific agent to be removed
        let agent_id_for_db = Self::db_agent_id(agent_id, profile)?;
        let agent_key = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", agent_id_for_db);
    
        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }
    
        // Delete the specific agent key
        self.db.delete_cf(cf_node_and_users, agent_key.as_bytes())?;
    
        Ok(())
    }

    pub fn update_agent_access(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
        new_profiles_with_access: Option<Vec<String>>,
        new_toolkits_accessible: Option<Vec<String>>,
    ) -> Result<(), ShinkaiDBError> {
        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let agent_id_for_db = Self::db_agent_id(agent_id, profile)?;
        let agent_key = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", agent_id_for_db);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_key.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);

        // Create a new RocksDB WriteBatch
        let mut batch = rocksdb::WriteBatch::default();

        // Directly update profiles_with_access if provided
        if let Some(profiles) = new_profiles_with_access {
            // Clear existing profiles for this agent and profile
            let existing_profiles_prefix = format!(
                "agent_{}_profile_{}",
                agent_id_for_db_hash,
                profile.get_profile_name_string().unwrap_or_default()
            );
            batch.delete_cf(cf_node_and_users, &existing_profiles_prefix);

            // Add new profiles access
            for profile_access in profiles {
                let profile_key = format!("agent_{}_profile_{}", agent_id_for_db_hash, profile_access);
                batch.put_cf(cf_node_and_users, &profile_key, "".as_bytes());
            }
        }

        // Directly update toolkits_accessible if provided
        if let Some(toolkits) = new_toolkits_accessible {
            // Clear existing toolkits for this agent and profile
            let existing_toolkits_prefix = format!(
                "agent_{}_toolkit_{}",
                agent_id_for_db_hash,
                profile.get_profile_name_string().unwrap_or_default()
            );
            batch.delete_cf(cf_node_and_users, &existing_toolkits_prefix);

            // Add new toolkits access
            for toolkit_access in toolkits {
                let toolkit_key = format!("agent_{}_toolkit_{}", agent_id_for_db_hash, toolkit_access);
                batch.put_cf(cf_node_and_users, &toolkit_key, "".as_bytes());
            }
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_agent(&self, agent_id: &str, profile: &ShinkaiName) -> Result<Option<SerializedLLMProvider>, ShinkaiDBError> {
        let agent_id_for_db = Self::db_agent_id(agent_id, profile)?;

        // Fetch the agent's bytes by their prefixed id from the NodeAndUsers topic
        let prefixed_id = format!("agent_placeholder_value_to_match_prefix_abcdef_{}", agent_id_for_db);
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_bytes = self.db.get_cf(cf_node_and_users, prefixed_id.as_bytes())?;

        // If the agent was found, deserialize the bytes into an agent object and return it
        if let Some(bytes) = agent_bytes {
            let agent: SerializedLLMProvider = from_slice(&bytes)?;
            Ok(Some(agent))
        } else {
            Err(ShinkaiDBError::DataNotFound)
        }
    }

    pub fn get_agent_profiles_with_access(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(agent_id, profile)?;
        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);

        let prefix = format!("agent_{}_profile_", agent_id_for_db_hash);
        let mut profiles_with_access = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Extract profile name from the key
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    // Ensure the key follows the prefix convention before extracting the profile name
                    if key_str.starts_with(&prefix) {
                        if let Some(profile_name) = key_str.split('_').last() {
                            profiles_with_access.push(profile_name.to_string());
                        }
                    }
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(profiles_with_access)
    }

    pub fn get_agent_toolkits_accessible(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(agent_id, profile)?;
        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);
        let prefix = format!("agent_{}_toolkit_", agent_id_for_db_hash);
        let mut toolkits_accessible = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Extract toolkit name from the key
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    // Ensure the key follows the prefix convention before extracting the toolkit name
                    if key_str.starts_with(&prefix) {
                        if let Some(toolkit_name) = key_str.split('_').last() {
                            toolkits_accessible.push(toolkit_name.to_string());
                        }
                    }
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(toolkits_accessible)
    }

    pub fn remove_profile_from_agent_access(
        &self,
        agent_id: &str,
        profile: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(agent_id, bounded_profile)?;
        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);
        let profile_key = format!("agent_{}_profile_{}", agent_id_for_db_hash, profile);

        // Delete the specific profile access key using native RocksDB method
        self.db.delete_cf(cf_node_and_users, profile_key.as_bytes())?;

        Ok(())
    }

    pub fn remove_toolkit_from_agent_access(
        &self,
        agent_id: &str,
        toolkit: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(agent_id, bounded_profile)?;
        let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);
        let toolkit_key = format!("agent_{}_toolkit_{}", agent_id_for_db_hash, toolkit);

        // Delete the specific toolkit access key using native RocksDB method
        self.db.delete_cf(cf_node_and_users, toolkit_key.as_bytes())?;

        Ok(())
    }

    pub fn get_agents_for_profile(&self, profile_name: ShinkaiName) -> Result<Vec<SerializedLLMProvider>, ShinkaiDBError> {
        let profile = profile_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::DataConversionError(
                "Profile name not found".to_string(),
            ))?;
        let mut result = Vec::new();

        // Assuming get_all_agents fetches all agents from the NodeAndUsers topic
        let all_agents = self.get_all_agents()?;

        // Iterate over all agents
        for agent in all_agents {
            let agent_id_for_db = Self::db_agent_id(&agent.id, &profile_name)?;
            let agent_id_for_db_hash = Self::agent_id_to_hash(&agent_id_for_db);

            // Construct the prefix to search for profiles with access to this agent
            let prefix = format!("agent_{}_profile_", agent_id_for_db_hash);
            let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

            // Use the prefix iterator to find all profiles with access to this agent
            let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix.as_bytes());
            let mut has_access = false;
            for item in iter {
                match item {
                    Ok((key, _)) => {
                        // Extract profile name from the key
                        let key_str = String::from_utf8(key.to_vec())
                            .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                        // Check if the extracted profile name matches the input profile name
                        if key_str.ends_with(&format!("_{}", profile)) {
                            has_access = true;
                            break;
                        }
                    }
                    Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
                }
            }

            // If the profile has access to the agent, add the agent to the result
            if has_access {
                result.push(agent);
            }
        }

        Ok(result)
    }
}
