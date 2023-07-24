use crate::db::{ShinkaiDB, Topic};
use crate::resources::document::DocumentResource;
use crate::resources::resource::Resource;
use crate::resources::router::ResourceRouter;
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, Options, DB};
use serde_json::{from_str, to_string};

use super::db_errors::ShinkaiDBError;

impl ShinkaiDB {
    /// Saves the `ResourceRouter` into the ShinkaiDB in the resources topic as
    /// a JSON string using the default key.
    fn save_resource_router(&self, router: &ResourceRouter) -> Result<(), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = router.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.db.put_cf(cf, ResourceRouter::db_key(), bytes)?;

        Ok(())
    }

    /// Saves the `Resource` into the ShinkaiDB in the resources topic as a JSON
    /// string.
    ///
    /// Note this is only to be used internally, as this does not add a resource
    /// pointer in the ResourceRouter. Adding the pointer is required for any
    /// resource being saved.
    fn save_resource_pointerless(&self, resource: &Box<dyn Resource>) -> Result<(), ShinkaiDBError> {
        // Convert Resource JSON to bytes for storage
        let json = resource.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.db.put_cf(cf, resource.db_key(), bytes)?;

        Ok(())
    }

    /// Saves the list of `Resource`s into the ShinkaiDB. This updates the
    /// Resource Router with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resources(&self, resources: Vec<Box<dyn Resource>>) -> Result<(), ShinkaiDBError> {
        // Get the resource router
        let mut router = self.get_resource_router()?;

        // TODO: Batch saving the resource and the router together
        // to guarantee atomicity and coherence of router.
        for resource in resources {
            // Save the JSON of the resources in the DB
            self.save_resource_pointerless(&resource)?;
            // Add the pointer to the router, saving the router
            // to the DB on each iteration
            router.add_resource_pointer(&resource);
            self.save_resource_router(&router)?;
        }

        // Add logic here for dealing with the resource router

        Ok(())
    }

    /// Fetches the Resource from the DB using the provided key
    /// in the resources topic, and parses it into a DocumentResource
    pub fn get_resource<K: AsRef<[u8]>>(&self, key: K) -> Result<DocumentResource, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, key)?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentResource object
        let document_resource: DocumentResource = from_str(json_str)?;

        Ok(document_resource)
    }

    /// Fetches the Resource Router from the `resource_router` key
    /// in the resources topic, and parses it into a ResourceRouter
    pub fn get_resource_router(&self) -> Result<ResourceRouter, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, ResourceRouter::db_key())?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentResource object
        let router: ResourceRouter = from_str(json_str)?;

        Ok(router)
    }
}
