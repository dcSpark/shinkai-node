use super::{db::Topic, db_errors::ShinkaiDBError, db_profile_bound::ProfileBoundWriteBatch, ShinkaiDB};
use rocksdb::{Error, Options};
use serde_json::{from_slice, to_vec};
use shinkai_message_primitives::schemas::{agents::serialized_agent::SerializedAgent, shinkai_name::ShinkaiName};

impl ShinkaiDB {
    // Fetches all agents from the Agents topic
    pub fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let mut result = Vec::new();
        let prefix = b"agent_";

        let iter = self.db.prefix_iterator_cf(cf, prefix);
        for item in iter {
            match item {
                Ok((_, value)) => {
                    let agent: SerializedAgent = from_slice(value.as_ref()).unwrap();
                    result.push(agent);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(result)
    }

    pub fn db_agent_id(agent_id: &str, profile: &ShinkaiName) -> Result<String, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .clone()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile.full_name.to_string()))?;

        Ok(format!("{}:::{}", agent_id, profile_name))
    }

    pub fn add_agent(&mut self, agent: SerializedAgent, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Serialize the agent to bytes
        let bytes = to_vec(&agent).unwrap();

        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let agent_id_for_db = Self::db_agent_id(&agent.id, profile)?;

        // Create a new RocksDB WriteBatch
        let mut batch = rocksdb::WriteBatch::default();

        // Prefix the agent ID and write it to the NodeAndUsers topic
        let agent_key = format!("agent_{}", &agent_id_for_db);
        batch.put_cf(cf_node_and_users, agent_key.as_bytes(), &bytes);

        // Additionally, for each allowed message sender and toolkit permission,
        // you can store them with a specific prefix to indicate their relationship to the agent.
        for profile in &agent.allowed_message_senders {
            let profile_key = format!("agent_{}_profile_{}", &agent_id_for_db, profile);
            batch.put_cf(cf_node_and_users, profile_key.as_bytes(), &[]);
        }
        for toolkit in &agent.toolkit_permissions {
            let toolkit_key = format!("agent_{}_toolkit_{}", &agent_id_for_db, toolkit);
            batch.put_cf(cf_node_and_users, toolkit_key.as_bytes(), &[]);
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_agent(&mut self, agent_id: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Get cf handle for NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Prefix used to identify all keys related to the agent
        let agent_id_for_db = Self::db_agent_id(&agent_id, profile)?;
        eprintln!("agent_id_for_db during remove: {}", agent_id_for_db);
        let agent_prefix = format!("agent_{}", agent_id_for_db);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_prefix.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        // Create a new RocksDB WriteBatch
        let mut batch = rocksdb::WriteBatch::default();

        // Iterate over all keys with the agent prefix to remove them
        let iter = self.db.prefix_iterator_cf(cf_node_and_users, agent_prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Convert the key from bytes to a UTF-8 string
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    eprintln!("key for removing: {:?}", key_str);
                    batch.delete_cf(cf_node_and_users, &key_str);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn update_agent_access(
        &mut self,
        agent_id: &str,
        profile: &ShinkaiName,
        new_profiles_with_access: Option<Vec<String>>,
        new_toolkits_accessible: Option<Vec<String>>,
    ) -> Result<(), ShinkaiDBError> {
        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let agent_id_for_db = Self::db_agent_id(&agent_id, profile)?;
        let agent_prefix = format!("agent_{}", agent_id_for_db);

        // Check if the agent exists
        let agent_exists = self.db.get_cf(cf_node_and_users, agent_prefix.as_bytes())?.is_some();
        if !agent_exists {
            return Err(ShinkaiDBError::DataNotFound);
        }

        // Create a new RocksDB WriteBatch
        let mut batch = rocksdb::WriteBatch::default();

        // Directly update profiles_with_access if provided
        if let Some(profiles) = new_profiles_with_access {
            // Clear existing profiles for this agent and profile
            let existing_profiles_prefix = format!(
                "agent_{}_profile_{}",
                agent_id_for_db,
                profile.get_profile_name().unwrap_or_default()
            );
            batch.delete_cf(cf_node_and_users, &existing_profiles_prefix);

            // Add new profiles access
            for profile_access in profiles {
                let profile_key = format!("agent_{}_profile_{}", agent_id_for_db, profile_access);
                batch.put_cf(cf_node_and_users, &profile_key, "".as_bytes());
            }
        }

        // Directly update toolkits_accessible if provided
        if let Some(toolkits) = new_toolkits_accessible {
            // Clear existing toolkits for this agent and profile
            let existing_toolkits_prefix = format!(
                "agent_{}_toolkit_{}",
                agent_id_for_db,
                profile.get_profile_name().unwrap_or_default()
            );
            batch.delete_cf(cf_node_and_users, &existing_toolkits_prefix);

            // Add new toolkits access
            for toolkit_access in toolkits {
                let toolkit_key = format!("agent_{}_toolkit_{}", agent_id_for_db, toolkit_access);
                batch.put_cf(cf_node_and_users, &toolkit_key, "".as_bytes());
            }
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_agent(&self, agent_id: &str, profile: &ShinkaiName) -> Result<Option<SerializedAgent>, ShinkaiDBError> {
        let agent_id_for_db = Self::db_agent_id(&agent_id, profile)?;
        eprintln!("agent_id_for_db: {}", agent_id_for_db);

        // Fetch the agent's bytes by their prefixed id from the NodeAndUsers topic
        let prefixed_id = format!("agent_{}", agent_id_for_db);
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_bytes = self.db.get_cf(cf_node_and_users, prefixed_id.as_bytes())?;

        // If the agent was found, deserialize the bytes into an agent object and return it
        if let Some(bytes) = agent_bytes {
            let agent: SerializedAgent = from_slice(&bytes)?;
            eprintln!("agent: {:?}", agent);
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
        let agent_id_for_db = Self::db_agent_id(&agent_id, profile)?;
        let prefix = format!("agent_{}_profile_", agent_id_for_db);
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
                        if let Some(profile_name) = key_str.split("_").last() {
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
        let agent_id_for_db = Self::db_agent_id(&agent_id, profile)?;
        let prefix = format!("agent_{}_toolkit_", agent_id_for_db);
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
                        if let Some(toolkit_name) = key_str.split("_").last() {
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
        &mut self,
        agent_id: &str,
        profile: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(&agent_id, bounded_profile)?;
        let profile_key = format!("agent_{}_profile_{}", agent_id_for_db, profile);

        // Delete the specific profile access key using native RocksDB method
        self.db.delete_cf(cf_node_and_users, profile_key.as_bytes())?;

        Ok(())
    }

    pub fn remove_toolkit_from_agent_access(
        &mut self,
        agent_id: &str,
        toolkit: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let agent_id_for_db = Self::db_agent_id(&agent_id, bounded_profile)?;
        let toolkit_key = format!("agent_{}_toolkit_{}", agent_id_for_db, toolkit);

        // Delete the specific toolkit access key using native RocksDB method
        self.db.delete_cf(cf_node_and_users, toolkit_key.as_bytes())?;

        Ok(())
    }

    pub fn get_agents_for_profile(&self, profile_name: ShinkaiName) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let profile = profile_name
            .get_profile_name()
            .ok_or(ShinkaiDBError::DataConversionError(
                "Profile name not found".to_string(),
            ))?;
        let mut result = Vec::new();

        // Assuming get_all_agents fetches all agents from the NodeAndUsers topic
        let all_agents = self.get_all_agents()?;

        // Iterate over all agents to check if the profile has access
        // TODO: this could be done more efficiently with a RocksDB iterator over a prefix
        for agent in all_agents {
            let agent_id_for_db = Self::db_agent_id(&agent.id, &profile_name)?;
            let access_key = format!("agent_{}_profile_{}", agent_id_for_db, profile);
            // Check if the access key exists in the NodeAndUsers topic
            let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
            let access_bytes = self.db.get_cf(cf_node_and_users, access_key.as_bytes())?;

            if access_bytes.is_some() {
                // If the profile has access to the agent, add the agent to the result
                result.push(agent);
            } else {
                // Optionally, check for other conditions like creation access
                if profile_name.contains(&agent.full_identity_name) {
                    result.push(agent);
                }
            }
        }

        Ok(result)
    }
}
