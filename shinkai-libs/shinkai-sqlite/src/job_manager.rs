use shinkai_message_primitives::schemas::job::Job;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn get_job_with_options(
        &self,
        job_id: &str,
        fetch_step_history: bool,
        fetch_scope_with_files: bool,
    ) -> Result<Job, SqliteManagerError> {
        unimplemented!()
    }
}
