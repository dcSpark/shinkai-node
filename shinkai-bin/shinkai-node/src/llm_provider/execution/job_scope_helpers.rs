use crate::llm_provider::job_manager::JobManager;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use std::result::Result::Ok;


impl JobManager {
    /// Retrieves all resources in the given job scope and returns them as a vector.
    pub async fn retrieve_all_resources_in_job_scope(
        scope: &MinimalJobScope,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<BaseVectorResource>, SqliteManagerError> {
        let mut resources = Vec::new();

        // Retrieve each file in the job scope
        for fs_item in &scope.vector_fs_items {
            let path = fs_item.path.clone();

            // Retrieve the processed file and add it to the resources vector
            match sqlite_manager.get_parsed_file_by_rel_path(&path.to_string()) {
                Ok(Some(parsed_file)) => {
                    let resource = BaseVectorResource::from(parsed_file);
                    resources.push(resource);
                }
                Ok(None) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!("File not found in database: {}", path),
                    );
                }
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!("Error retrieving file from database: {} with error: {:?}", path, e),
                    );
                    return Err(e);
                }
            }
        }

        Ok(resources)
    }
}

impl JobManager {
    /// TODO: When Folder is added into job scope, count the number of folders/items at the time and store in job scope
    /// Counts the number of resources in the scope, accessing the VectorFS to check for folders
    pub async fn count_number_of_resources_in_job_scope(
        profile: &ShinkaiName,
        scope: &MinimalJobScope,
    ) -> Result<usize, SqliteManagerError> {
        let mut count = scope.local_vrkai.len() + scope.vector_fs_items.len();

        for vrpack_entry in &scope.local_vrpack {
            count += vrpack_entry.vrpack.vrkai_count as usize;
        }
        for folder in &scope.vector_fs_folders {
            let path = folder.path.clone();
            let folder_content = vector_fs.count_number_of_items_under_path(path, profile).await?;
            count += folder_content;
        }

        Ok(count)
    }

    /// Helper method which fetches all local VRs, & directly linked FSItem Vector Resources specified in the given JobScope.
    /// Returns all of them in a single list ready to be used.
    /// Of note, this does not fetch resources inside of folders in the job scope, as those are not fetched in whole,
    /// but instead have a deep vector search performed on them via the VectorFS itself separately.
    pub async fn fetch_job_scope_direct_resources(
        scope: &MinimalJobScope,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, SqliteManagerError> {
        let mut resources = Vec::new();

        // Add local resources to the list
        for local_entry in &job_scope.local_vrkai {
            resources.push(local_entry.vrkai.resource.clone());
        }

        for fs_item in &job_scope.vector_fs_items {
            let reader = vector_fs
                .new_reader(profile.clone(), fs_item.path.clone(), profile.clone())
                .await
                .map_err(|e| SqliteManagerError::SomeError(format!("VectorFS error: {}", e)))?;

            let ret_resource = vector_fs
                .retrieve_vector_resource(&reader)
                .await
                .map_err(|e| SqliteManagerError::SomeError(format!("VectorFS error: {}", e)))?;
            resources.push(ret_resource);
        }

        Ok(resources)
    }
}
