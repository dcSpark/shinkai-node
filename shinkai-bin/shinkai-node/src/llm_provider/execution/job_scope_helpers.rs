use crate::llm_provider::job_manager::JobManager;
use futures::stream::Stream;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_vector_fs::vector_fs::{vector_fs::VectorFS, vector_fs_error::VectorFSError};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, VRPath};
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

struct ResourceStream {
    receiver: mpsc::Receiver<BaseVectorResource>,
}

impl Stream for ResourceStream {
    type Item = BaseVectorResource;

    /// Polls the next BaseVectorResource from the stream
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_recv(cx)
    }
}

impl JobManager {
    /// Retrieves all resources in the given job scope, returning them as a stream. Starts with VRKai, then VRPacks, then VectorFS items, then VectorFS folders.
    /// For VectorFS items/folders, only fetches 5 at a time, to save on memory + for it to be more agile.
    pub async fn retrieve_all_resources_in_job_scope_stream(
        vector_fs: Arc<VectorFS>,
        scope: &JobScope,
        profile: &ShinkaiName,
    ) -> impl Stream<Item = BaseVectorResource> {
        let (tx, rx) = mpsc::channel(5);

        // Extract all of the vrkai and vrpack BaseVectorResources (cloned)
        let vrkai_resources = scope
            .local_vrkai
            .iter()
            .map(|e| e.vrkai.resource.clone())
            .collect::<Vec<BaseVectorResource>>();
        let vrpacks_resources = {
            let mut resources = Vec::new();
            for entry in &scope.local_vrpack {
                if let Ok(unpacked_vrkais) = entry.vrpack.unpack_all_vrkais() {
                    for (vrkai, _) in unpacked_vrkais {
                        resources.push(vrkai.resource);
                    }
                }
            }
            resources
        };

        // Get the entries for FS folders/items
        let fs_item_paths = scope
            .vector_fs_items
            .iter()
            .map(|e| e.path.clone())
            .collect::<Vec<VRPath>>();
        let fs_folder_paths = scope
            .vector_fs_folders
            .iter()
            .map(|e| e.path.clone())
            .collect::<Vec<VRPath>>();
        let cloned_profile1 = profile.clone();

        tokio::spawn(async move {
            // Iterate over local VRKai resources
            for resource in vrkai_resources {
                if tx.send(resource).await.is_err() {
                    // Handle the case where the receiver has been dropped
                    break;
                }
            }

            // Iterate over local VRPacks resources
            for resource in vrpacks_resources {
                if tx.send(resource).await.is_err() {
                    break;
                }
            }

            // Iterate over vector_fs_items, fetching resources asynchronously
            for path in fs_item_paths {
                let reader = match vector_fs
                    .new_reader(cloned_profile1.clone(), path.clone(), cloned_profile1.clone())
                    .await
                {
                    Ok(reader) => reader,
                    Err(_) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            &format!("retrieve_all_resources_in_job_scope reader create failed: {}", path),
                        );
                        continue;
                    }
                };

                match vector_fs.retrieve_vector_resource(&reader).await {
                    Ok(resource) => {
                        if tx.send(resource).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            &format!("retrieve_all_resources_in_job_scope VR retrieve failed: {}", path),
                        );
                        continue;
                    }
                }
            }

            // Iterate over vector fs folders, fetching resources asynchronously
            for folder_path in fs_folder_paths {
                let folder_reader = match vector_fs
                    .new_reader(cloned_profile1.clone(), folder_path.clone(), cloned_profile1.clone())
                    .await
                {
                    Ok(reader) => reader,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            &format!(
                                "retrieve_all_resources_in_job_scope reader create failed: {} with error: {:?}",
                                folder_path, e
                            ),
                        );
                        continue;
                    }
                };

                // Fetch resource paths
                let resource_paths = match vector_fs.retrieve_all_item_paths_underneath_folder(folder_reader).await {
                    Ok(paths) => paths,
                    Err(_) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            &format!(
                                "retrieve_all_resources_in_job_scope VRs paths in folder fetching failed: {}",
                                folder_path
                            ),
                        );

                        continue;
                    }
                };

                // Now start processing each resource in the folder
                for resource_path in resource_paths {
                    let resource_reader = match vector_fs
                        .new_reader(cloned_profile1.clone(), resource_path.clone(), cloned_profile1.clone())
                        .await
                    {
                        Ok(reader) => reader,
                        Err(_) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "retrieve_all_resources_in_job_scope reader create failed: {}",
                                    folder_path
                                ),
                            );
                            continue;
                        }
                    };

                    match vector_fs.retrieve_vector_resource(&resource_reader).await {
                        Ok(resource) => {
                            if tx.send(resource).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "retrieve_all_resources_in_job_scope VR retrieve failed: {}",
                                    resource_path
                                ),
                            );
                            continue;
                        }
                    }
                }
            }
        });

        ResourceStream { receiver: rx }
    }
}

impl JobManager {
    /// TODO: When Folder is added into job scope, count the number of folders/items at the time and store in job scope
    /// Counts the number of resources in the scope, accessing the VectorFS to check for folders
    pub async fn count_number_of_resources_in_job_scope(
        vector_fs: Arc<VectorFS>,
        profile: &ShinkaiName,
        scope: &JobScope,
    ) -> Result<usize, VectorFSError> {
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
        vector_fs: Arc<VectorFS>,
        job_scope: &JobScope,
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
