use rocksdb::IteratorMode;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use crate::network::node_proxy::ProxyIdentity;

use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use std::{collections::HashMap, net::SocketAddr};

impl ShinkaiDB {
    pub fn insert_proxied_identities(
        &self,
        allow_new_identities: bool,
        proxy_node_identities: HashMap<String, ProxyIdentity>,
    ) -> Result<(), ShinkaiDBError> {
        // Retrieve the handle to the "ProxyIdentities" column family
        let proxy_identities_cf = self.get_cf_handle(Topic::ProxyIdentities).unwrap();

        if !allow_new_identities {
            // If allow_new_identities is false, remove everything previously saved in the column ProxyIdentities.
            let iter = self.db.iterator_cf(proxy_identities_cf, IteratorMode::Start);
            for item in iter {
                let (key, _) = item.map_err(ShinkaiDBError::from)?;
                self.db.delete_cf(proxy_identities_cf, key)?;
            }
        }

        // Add the new identities to the ProxyIdentities column family
        for (key, identity) in proxy_node_identities {
            // Serialize ProxyIdentity into bytes for storage
            let identity_bytes = bincode::serialize(&identity).map_err(|_| ShinkaiDBError::InvalidData)?;

            // Insert the ProxyIdentity into the "ProxyIdentities" column family using the key
            self.db.put_cf(proxy_identities_cf, key, identity_bytes)?;
        }

        Ok(())
    }

    pub fn get_proxied_identity(&self, shinkai_name: &ShinkaiName) -> Result<Option<ProxyIdentity>, ShinkaiDBError> {
        // Retrieve the handle to the "ProxyIdentities" column family
        let proxy_identities_cf = self.get_cf_handle(Topic::ProxyIdentities).unwrap();

        // Get the node name from the ShinkaiName, which will be used as the key
        let key = shinkai_name.get_node_name_string();

        // Get the value associated with the key from the "ProxyIdentities" column family
        match self.db.get_cf(proxy_identities_cf, key)? {
            Some(value) => {
                // If a value is found, deserialize it into a ProxyIdentity
                let identity: ProxyIdentity = bincode::deserialize(&value).map_err(|_| ShinkaiDBError::InvalidData)?;
                Ok(Some(identity))
            }
            None => Ok(None), // If no value is found, return None
        }
    }

    pub fn insert_my_proxy(&self, my_proxy: ProxyIdentity) -> Result<(), ShinkaiDBError> {
        // Retrieve the handle to the "MyProxy" column family
        let my_proxy_cf = self.get_cf_handle(Topic::MyProxy).unwrap();

        // Serialize ProxyIdentity into bytes for storage
        let my_proxy_bytes = bincode::serialize(&my_proxy).map_err(|_| ShinkaiDBError::InvalidData)?;

        // Insert the ProxyIdentity into the "MyProxy" column family using a constant key
        // Here we use "myproxy" as the key, but it could be any string that makes sense in your application
        self.db.put_cf(my_proxy_cf, "myproxy", my_proxy_bytes)?;

        Ok(())
    }

    pub fn get_my_proxy(&self) -> Result<Option<ProxyIdentity>, ShinkaiDBError> {
        // Retrieve the handle to the "MyProxy" column family
        let my_proxy_cf = self.get_cf_handle(Topic::MyProxy).unwrap();

        // Get the value associated with the key "myproxy" from the "MyProxy" column family
        match self.db.get_cf(my_proxy_cf, "myproxy")? {
            Some(value) => {
                // If a value is found, deserialize it into a ProxyIdentity
                let my_proxy: ProxyIdentity = bincode::deserialize(&value).map_err(|_| ShinkaiDBError::InvalidData)?;
                Ok(Some(my_proxy))
            }
            None => Ok(None), // If no value is found, return None
        }
    }
}
