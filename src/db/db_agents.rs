use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::{agent::Agent, agent_serialization::SerializedAgent};
use rocksdb::{Error, Options};
use serde_json::{from_slice, to_vec};

impl ShinkaiDB {
    // Fetches all agents from the Agents topic
    pub fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, Error> {
        let cf = self.db.cf_handle(Topic::Agents.as_str()).unwrap();
        let mut result = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            match item {
                Ok((_, value)) => {
                    let agent: SerializedAgent = from_slice(value.as_ref()).unwrap();
                    result.push(agent);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(result)
    }

    pub fn add_agent(&mut self, agent: SerializedAgent) -> Result<(), ShinkaiDBError> {
        // Create Options for ColumnFamily
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        // Create ColumnFamilyDescriptors for profiles_with_access and toolkits_accessible
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent.id);
        let cf_name_toolkits_accessible = format!("agent_{}_toolkits_accessible", agent.id);

        // Create column families
        self.db.create_cf(&cf_name_profiles_access, &cf_opts)?;
        self.db.create_cf(&cf_name_toolkits_accessible, &cf_opts)?;

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Get handles to the newly created column families
        let cf_profiles_access = self.db.cf_handle(&cf_name_profiles_access).unwrap();
        let cf_toolkits_accessible = self.db.cf_handle(&cf_name_toolkits_accessible).unwrap();

        // Write profiles_with_access and toolkits_accessible to respective columns
        for profile in &agent.allowed_message_senders {
            batch.put_cf(cf_profiles_access, &profile, "".as_bytes());
        }
        for toolkit in &agent.toolkit_permissions {
            batch.put_cf(cf_toolkits_accessible, &toolkit, "".as_bytes());
        }

        // Serialize the agent to bytes and write it to the Agents topic
        let bytes = to_vec(&agent).unwrap();
        let cf_agents = self.db.cf_handle(Topic::Agents.as_str()).unwrap();
        batch.put_cf(cf_agents, agent.id.as_bytes(), &bytes);

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_agent(&mut self, agent_id: &str) -> Result<(), ShinkaiDBError> {
        // Define the unique column family names
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent_id);
        let cf_name_toolkits_accessible = format!("agent_{}_toolkits_accessible", agent_id);

        // Get cf handle for Agents topic
        let cf_agents = self.db.cf_handle(Topic::Agents.as_str()).unwrap();

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Remove the agent from the Agents topic
        batch.delete_cf(cf_agents, agent_id.as_bytes());

        // Remove the agent's access profiles and toolkits
        // This involves deleting the column families entirely
        self.db.drop_cf(&cf_name_profiles_access)?;
        self.db.drop_cf(&cf_name_toolkits_accessible)?;

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn update_agent_access(
        &mut self,
        agent_id: &str,
        new_profiles_with_access: Option<Vec<String>>,
        new_toolkits_accessible: Option<Vec<String>>,
    ) -> Result<(), ShinkaiDBError> {
        // Define the unique column family names
        let cf_name_profiles_access = format!("agent_{}_profiles_with_access", agent_id);
        let cf_name_toolkits_access = format!("agent_{}_toolkits_accessible", agent_id);

        // Get the column families. They should have been created when the agent was added.
        let cf_profiles_access = self
            .db
            .cf_handle(&cf_name_profiles_access)
            .ok_or(ShinkaiDBError::SomeError)?;
        let cf_toolkits_access = self
            .db
            .cf_handle(&cf_name_toolkits_access)
            .ok_or(ShinkaiDBError::SomeError)?;

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Update profiles_with_access if new_profiles_with_access is provided
        if let Some(profiles) = new_profiles_with_access {
            for profile in profiles {
                batch.put_cf(cf_profiles_access, profile.as_bytes(), "".as_bytes());
            }
        }

        // Update toolkits_accessible if new_toolkits_accessible is provided
        if let Some(toolkits) = new_toolkits_accessible {
            for toolkit in toolkits {
                batch.put_cf(cf_toolkits_access, toolkit.as_bytes(), "".as_bytes());
            }
        }

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<Option<SerializedAgent>, ShinkaiDBError> {
        // Get cf handle for Agents topic
        let cf_agents = self.db.cf_handle(Topic::Agents.as_str()).unwrap();
    
        // Fetch the agent's bytes by their id from the Agents topic
        let agent_bytes = self.db.get_cf(cf_agents, agent_id.as_bytes())?;
    
        // If the agent was found, deserialize the bytes into an agent object and return it
        match agent_bytes {
            Some(bytes) => {
                let agent: SerializedAgent = from_slice(&bytes)?;
                Ok(Some(agent))
            },
            None => Ok(None),  // If the agent wasn't found, return None
        }
    }

     pub fn get_agent_profiles_with_access(&self, agent_id: &str) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_name = format!("agent_{}_profiles_with_access", agent_id);
        let cf = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::SomeError)?;
        self.get_column_family_data(cf)
    }

    pub fn get_agent_toolkits_accessible(&self, agent_id: &str) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_name = format!("agent_{}_toolkits_accessible", agent_id);
        let cf = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::SomeError)?;
        self.get_column_family_data(cf)
    }

    pub fn remove_profile_from_agent_access(&mut self, agent_id: &str, profile: &str) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("agent_{}_profiles_with_access", agent_id);
        let cf = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::SomeError)?;

        self.db.delete_cf(cf, profile.as_bytes())?;
        Ok(())
    }

    pub fn remove_toolkit_from_agent_access(&mut self, agent_id: &str, toolkit: &str) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("agent_{}_toolkits_accessible", agent_id);
        let cf = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::SomeError)?;

        self.db.delete_cf(cf, toolkit.as_bytes())?;
        Ok(())
    }

    fn get_column_family_data(&self, cf: &rocksdb::ColumnFamily) -> Result<Vec<String>, ShinkaiDBError> {
        let mut data = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8(key.to_vec()).map_err(|_| ShinkaiDBError::SomeError)?;
                    data.push(key_str);
                }
                Err(_) => return Err(ShinkaiDBError::SomeError),
            }
        }

        Ok(data)
    } 
}
