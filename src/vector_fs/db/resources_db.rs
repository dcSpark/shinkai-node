use super::super::fs_error::VectorFSError;
use super::fs_db::{FSTopic, VectorFSDB};
use crate::db::db::ProfileBoundWriteBatch;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::vector_search_traversal::VRHeader;

impl VectorFSDB {
    /// Saves the `VectorResource` into the Resources topic as a JSON
    /// string. Note: this is only to be used internally, as this simply saves the resource to the FSDB,
    /// and does absolutely nothing else related to the VectorFS.
    pub fn wb_save_resource(
        &self,
        resource: &BaseVectorResource,
        batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        let (bytes, cf) = self._prepare_resource(resource)?;

        // Insert into the "VectorResources" column family
        batch.put_cf_pb(cf, &resource.as_trait_object().reference_string(), &bytes);

        Ok(())
    }

    /// Prepares the `BaseVectorResource` for saving into the FSDB in the resources topic as a JSON
    /// string. Note this is only to be used internally.
    fn _prepare_resource(
        &self,
        resource: &BaseVectorResource,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), VectorFSError> {
        let json = resource.to_json()?;
        let bytes = json.as_bytes().to_vec();
        // Retrieve the handle for the "VectorResources" column family
        let cf = self.get_cf_handle(FSTopic::VectorResources)?;

        Ok((bytes, cf))
    }

    /// Fetches the BaseVectorResource from the DB using a VRHeader
    pub fn get_resource_by_header(
        &self,
        resource_header: &VRHeader,
        profile: &ShinkaiName,
    ) -> Result<BaseVectorResource, VectorFSError> {
        self.get_resource(&resource_header.reference_string(), profile)
    }

    /// Fetches the BaseVectorResource from the FSDB in the VectorResources topic
    pub fn get_resource(&self, key: &str, profile: &ShinkaiName) -> Result<BaseVectorResource, VectorFSError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(FSTopic::VectorResources, key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;
        Ok(BaseVectorResource::from_json(json_str)?)
    }
}
