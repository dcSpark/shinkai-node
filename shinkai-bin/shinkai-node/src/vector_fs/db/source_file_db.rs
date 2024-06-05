use super::super::vector_fs_error::VectorFSError;
use super::fs_db::{FSTopic, VectorFSDB};
use crate::db::db_profile_bound::ProfileBoundWriteBatch;
use crate::vector_fs::vector_fs_types::FSItem;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::source::SourceFileMap;
use shinkai_vector_resources::vector_resource::VRHeader;

impl VectorFSDB {
    /// Saves the `SourceFileMap` into the SourceFiles topic.
    pub fn wb_save_source_file_map(
        &self,
        source_file_map: &SourceFileMap,
        db_key: &str,
        batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        let (bytes, cf) = self._prepare_source_file_map(source_file_map)?;

        // Insert into the "SourceFileMaps" column family
        batch.pb_put_cf(cf, db_key, &bytes);

        Ok(())
    }

    /// Prepares the `SourceFileMap` for saving into the FSDB in the SourceFiles topic.
    fn _prepare_source_file_map(&self, source_file_map: &SourceFileMap) -> Result<(Vec<u8>, &str), VectorFSError> {
        let json = source_file_map.to_json()?;
        let bytes = json.as_bytes().to_vec();
        // Retrieve the handle for the "SourceFiles" column family
        let cf = FSTopic::SourceFiles.as_str();
        Ok((bytes, cf))
    }

    /// Fetches the SourceFileMap from the DB using a VRHeader
    pub fn get_source_file_map_by_header(
        &self,
        resource_header: &VRHeader,
        profile: &ShinkaiName,
    ) -> Result<SourceFileMap, VectorFSError> {
        self.get_source_file_map(&resource_header.reference_string(), profile)
    }

    /// Fetches the SourceFileMap from the DB using a FSItem
    pub fn get_source_file_map_by_fs_item(
        &self,
        fs_item: &FSItem,
        profile: &ShinkaiName,
    ) -> Result<SourceFileMap, VectorFSError> {
        let key = fs_item.source_file_map_db_key()?;
        self.get_source_file_map(&key, profile)
    }

    /// Fetches the SourceFileMap from the FSDB in the SourceFiles topic
    pub fn get_source_file_map(&self, key: &str, profile: &ShinkaiName) -> Result<SourceFileMap, VectorFSError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(FSTopic::SourceFiles, key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;
        Ok(SourceFileMap::from_json(json_str)?)
    }
}
