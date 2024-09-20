use super::super::vector_fs_error::VectorFSError;
use super::fs_db::{FSTopic, ProfileBoundWriteBatch, VectorFSDB};
use crate::vector_fs::vector_fs_types::FSItem;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, VRHeader};

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
        batch.pb_put_cf(cf, &resource.as_trait_object().reference_string(), &bytes);

        Ok(())
    }

    /// Prepares the `BaseVectorResource` for saving into the FSDB in the resources topic as a JSON
    /// string. Note this is only to be used internally.
    fn _prepare_resource(&self, resource: &BaseVectorResource) -> Result<(Vec<u8>, &str), VectorFSError> {
        let json = resource.to_json()?;
        let bytes = json.as_bytes().to_vec();
        // Retrieve the handle for the "VectorResources" column family
        let cf = FSTopic::VectorResources.as_str();

        Ok((bytes, cf))
    }

    /// Deletes the `VectorResource` from the Resources topic.
    /// Note: this is only to be used internally, as this simply removes the resource from the FSDB,
    /// and does absolutely nothing else related to the VectorFS.
    pub fn wb_delete_resource(
        &self,
        reference_string: &str,
        batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        // Delete from the "VectorResources" column family
        batch.pb_delete_cf(FSTopic::VectorResources.as_str(), reference_string);

        Ok(())
    }

    /// Fetches the BaseVectorResource from the DB using a VRHeader
    pub fn get_resource_by_header(
        &self,
        resource_header: &VRHeader,
        profile: &ShinkaiName,
    ) -> Result<BaseVectorResource, VectorFSError> {
        self.get_resource(&resource_header.reference_string(), profile)
    }

    /// Fetches the BaseVectorResource from the DB using a FSItem
    pub fn get_resource_by_fs_item(
        &self,
        fs_item: &FSItem,
        profile: &ShinkaiName,
    ) -> Result<BaseVectorResource, VectorFSError> {
        self.get_resource(&fs_item.resource_db_key(), profile)
    }

    /// Fetches the BaseVectorResource from the FSDB in the VectorResources topic
    pub fn get_resource(&self, key: &str, profile: &ShinkaiName) -> Result<BaseVectorResource, VectorFSError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(FSTopic::VectorResources, key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;
        Ok(BaseVectorResource::from_json(json_str)?)
    }
}
