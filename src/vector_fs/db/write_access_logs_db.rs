use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::VRPath;

use crate::{vector_fs::vector_fs_error::VectorFSError, db::db_profile_bound::ProfileBoundWriteBatch};

use super::fs_db::VectorFSDB;

impl VectorFSDB {
    /// TODO: Implement real logic
    /// Adds the write access log into the FSDB
    pub fn wb_add_write_access_log(
        &self,
        _requester_name: ShinkaiName,
        _write_path: &VRPath,
        _datetime: DateTime<Utc>,
        _profile: ShinkaiName,
        _batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        // let (bytes, cf) = self._prepare_write_access_log(...)?;
        // batch.put_cf_pb(cf, db_key, &bytes);

        // 1. Update the path_write_log_count in the db + 1

        // 2. Add the new access log into the +1 key

        Ok(())
    }

    /// TODO: Implement real logic
    /// Returns a tuple of Option<(DateTime<Utc>, ShinkaiName)>, of the last access
    /// made at a specific path. If was successful but returns None, then there were no
    /// write access logs stored in the DB.
    pub fn get_latest_write_access_log(
        &self,
        _write_path: &VRPath,
        _profile: ShinkaiName,
    ) -> Result<Option<(DateTime<Utc>, ShinkaiName)>, VectorFSError> {
        // Implement logic
        Ok(None)
    }
}
