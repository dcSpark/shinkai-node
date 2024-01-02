use super::super::vector_fs_error::VectorFSError;
use super::fs_db::{FSTopic, VectorFSDB};
use crate::db::db::ProfileBoundWriteBatch;
use crate::vector_fs::vector_fs_types::FSItem;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::source::SourceFile;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, VRHeader};

impl VectorFSDB {
    /// Saves the `SourceFile` into the SourceFiles topic.
    pub fn wb_save_source_file(
        &self,
        source_file: &SourceFile,
        db_key: &str,
        batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        let (bytes, cf) = self._prepare_source_file(source_file)?;

        // Insert into the "SourceFiles" column family
        batch.put_cf_pb(cf, db_key, &bytes);

        Ok(())
    }

    /// Prepares the `SourceFile` for saving into the FSDB in the SourceFiles topic.
    fn _prepare_source_file(
        &self,
        source_file: &SourceFile,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), VectorFSError> {
        let json = source_file.to_json()?;
        let bytes = json.as_bytes().to_vec();
        // Retrieve the handle for the "SourceFiles" column family
        let cf = self.get_cf_handle(FSTopic::SourceFiles)?;
        Ok((bytes, cf))
    }

    /// Fetches the SourceFile from the DB using a VRHeader
    pub fn get_source_file_by_header(
        &self,
        resource_header: &VRHeader,
        profile: &ShinkaiName,
    ) -> Result<SourceFile, VectorFSError> {
        self.get_source_file(&resource_header.reference_string(), profile)
    }

    /// Fetches the SourceFile from the DB using a FSItem
    pub fn get_source_file_by_fs_item(
        &self,
        fs_item: &FSItem,
        profile: &ShinkaiName,
    ) -> Result<SourceFile, VectorFSError> {
        let key = fs_item.source_file_db_key()?;
        self.get_source_file(&key, profile)
    }

    /// Fetches the SourceFile from the FSDB in the SourceFiles topic
    pub fn get_source_file(&self, key: &str, profile: &ShinkaiName) -> Result<SourceFile, VectorFSError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(FSTopic::SourceFiles, key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;
        Ok(SourceFile::from_json(json_str)?)
    }
}
