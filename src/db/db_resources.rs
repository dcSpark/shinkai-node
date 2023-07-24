use crate::db::{ShinkaiDB, Topic};
use crate::resources::document::DocumentResource;
use crate::resources::resource::Resource;
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, Options, DB};
use serde_json::{from_str, to_string};

use super::db_errors::ShinkaiDBError;

impl ShinkaiDB {
    /// Saves the `Resource` into the ShinkaiDB in the resources topic as a JSON
    /// string using the Resource name as the key.
    fn save_resource_json(&self, resource: Box<dyn Resource>) -> Result<(), ShinkaiDBError> {
        // Convert Resource JSON to bytes for storage
        let json = resource.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.db.put_cf(cf, resource.db_key(), bytes)?;

        Ok(())
    }

    /// Saves the list of `Resource`s into the ShinkaiDB. This updates the
    /// Resource Router with the embeddings + name of the resources as well. Of
    /// note, if an existing resource exists in the DB with the same name,
    /// this will overwrite the old resource completely.
    pub fn save_resources(&self, resources: Vec<Box<dyn Resource>>) -> Result<(), ShinkaiDBError> {
        // Save the JSON of the resources in the DB under their name as the key
        for resource in resources {
            self.save_resource_json(resource)?;
        }

        let router = self.get_resource_router()?;

        // Add logic here for dealing with the resource router

        Ok(())
    }

    /// Fetches the Resource Router from the `resource_router` key
    /// in the resources topic, and parses it into a DocumentResource
    pub fn get_resource_router(&self) -> Result<DocumentResource, ShinkaiDBError> {
        let router_key = "resource_router";

        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, router_key)?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentResource object
        let document_resource: DocumentResource = from_str(json_str)?;

        Ok(document_resource)
    }
}
