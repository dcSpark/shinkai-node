use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB, db_profile_bound::ProfileBoundWriteBatch};
use rocksdb::{Error, Options};
use serde_json::{from_slice, to_vec};
use shinkai_message_primitives::schemas::{agents::serialized_agent::SerializedAgent, shinkai_name::ShinkaiName};

impl ShinkaiDB {
    // Fetches all agents from the Agents topic
    pub fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::Agents.as_str())?;
        let mut result = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
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

    pub fn add_agent(&mut self, agent: SerializedAgent, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Create Options for ColumnFamily
        let cf_opts = Self::create_cf_options();

        // Create ColumnFamilyDescriptors for profiles_with_access and toolkits_accessible
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent.id);
        let cf_name_toolkits_accessible = format!("agent_{}_toolkits_accessible", agent.id);

        // Create column families
        self.db.create_cf(&cf_name_profiles_access, &cf_opts)?;
        self.db.create_cf(&cf_name_toolkits_accessible, &cf_opts)?;

        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Get handles to the newly created column families
        let cf_profiles_access = self.cf_handle(&cf_name_profiles_access)?;
        let cf_toolkits_accessible = self.cf_handle(&cf_name_toolkits_accessible)?;

        // Write profiles_with_access and toolkits_accessible to respective columns
        for profile in &agent.allowed_message_senders {
            // TODO: this doesnt add up
            pb_batch.pb_put_cf(cf_profiles_access, &profile, "".as_bytes());
        }
        for toolkit in &agent.toolkit_permissions {
            pb_batch.pb_put_cf(cf_toolkits_accessible, &toolkit, "".as_bytes());
        }

        // Serialize the agent to bytes and write it to the Agents topic
        let bytes = to_vec(&agent).unwrap();
        let cf_agents = self.cf_handle(Topic::Agents.as_str())?;
        pb_batch.pb_put_cf(cf_agents, &agent.id, &bytes);

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn remove_agent(&mut self, agent_id: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Define the unique column family names
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent_id);
        let cf_name_toolkits_accessible = format!("agent_{}_toolkits_accessible", agent_id);

        // Get cf handle for Agents topic
        let cf_agents = self.cf_handle(Topic::Agents.as_str())?;

        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Remove the agent from the Agents topic
        pb_batch.pb_delete_cf(cf_agents, &agent_id);

        // Remove the agent's access profiles and toolkits
        // This involves deleting the column families entirely
        self.db.drop_cf(&cf_name_profiles_access)?;
        self.db.drop_cf(&cf_name_toolkits_accessible)?;

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn update_agent_access(
        &mut self,
        agent_id: &str,
        profile: &ShinkaiName,
        new_profiles_with_access: Option<Vec<String>>,
        new_toolkits_accessible: Option<Vec<String>>,
    ) -> Result<(), ShinkaiDBError> {
        // Define the unique column family names
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent_id);
        let cf_name_toolkits_access = format!("agent_{}_toolkits_accessible", agent_id);

        // Get the column families. They should have been created when the agent was added.
        let cf_profiles_access =
            self.db
                .cf_handle(&cf_name_profiles_access)
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                    "Column family not found for: {}",
                    cf_name_profiles_access
                )))?;
        let cf_toolkits_access =
            self.db
                .cf_handle(&cf_name_toolkits_access)
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                    "Column family not found for: {}",
                    cf_name_toolkits_access
                )))?;

        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Update profiles_with_access if new_profiles_with_access is provided
        if let Some(profiles) = new_profiles_with_access {
            for profile in profiles {
                pb_batch.pb_put_cf(cf_profiles_access, profile.as_str(), "".as_bytes());
            }
        }

        // Update toolkits_accessible if new_toolkits_accessible is provided
        if let Some(toolkits) = new_toolkits_accessible {
            for toolkit in toolkits {
                pb_batch.pb_put_cf(cf_toolkits_access, toolkit.as_str(), "".as_bytes());
            }
        }

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn get_agent(&self, agent_id: &str, profile: &ShinkaiName) -> Result<Option<SerializedAgent>, ShinkaiDBError> {
        // Fetch the agent's bytes by their id from the Agents topic
        let agent_bytes = self.pb_topic_get(Topic::Agents, agent_id, profile)?;

        // If the agent was found, deserialize the bytes into an agent object and return it
        let agent: SerializedAgent = from_slice(agent_bytes.as_slice())?;
        Ok(Some(agent))
    }

    pub fn get_agent_profiles_with_access(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        // Start write batch for atomic operation
        let cf_name = format!("agent_{}_profiles_with_access", agent_id);

        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                "Column family not found for: {}",
                cf_name
            )))?;
        self.pb_cf_get_all_keys(cf, profile)
    }

    pub fn get_agent_toolkits_accessible(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_name = format!("agent_{}_toolkits_accessible", agent_id);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                "Column family not found for: {}",
                cf_name
            )))?;
        self.pb_cf_get_all_keys(cf, profile)
    }

    pub fn remove_profile_from_agent_access(
        &mut self,
        agent_id: &str,
        profile: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("agent_{}_profiles_with_access", agent_id);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                "Column family not found for: {}",
                cf_name
            )))?;

        self.pb_delete_cf(cf, profile, &bounded_profile)?;
        Ok(())
    }

    pub fn remove_toolkit_from_agent_access(
        &mut self,
        agent_id: &str,
        toolkit: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("agent_{}_toolkits_accessible", agent_id);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(format!(
                "Column family not found for: {}",
                cf_name
            )))?;

        self.pb_delete_cf(cf, toolkit, bounded_profile)?;
        Ok(())
    }

    fn get_column_family_data(&self, cf: &rocksdb::ColumnFamily) -> Result<Vec<String>, ShinkaiDBError> {
        let mut data = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    data.push(key_str);
                }
                Err(_) => {
                    return Err(ShinkaiDBError::DataConversionError(
                        "Error iterating over column family".to_string(),
                    ))
                }
            }
        }

        Ok(data)
    }

    pub fn get_agents_for_profile(&self, profile_name: ShinkaiName) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let profile = profile_name
            .get_profile_name()
            .ok_or(ShinkaiDBError::DataConversionError(
                "Profile name not found".to_string(),
            ))?;
        let all_agents = self.get_all_agents()?;
        let mut result = Vec::new();

        for agent in all_agents {
            let cf_name = format!("agent_{}_profiles_with_access", agent.id);
            let cf = self.cf_handle(&cf_name)?;
            let profiles = self.get_column_family_data(cf)?;

            if profiles.contains(&profile) {
                result.push(agent);
            } else {
                // Check for creation access
                if profile_name.contains(&agent.full_identity_name) {
                    result.push(agent);
                }
            }
        }

        Ok(result)
    }
}
