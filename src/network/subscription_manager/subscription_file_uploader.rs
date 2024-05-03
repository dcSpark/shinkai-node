// List of potential providers
// - Cloudflare R2
// - AWS S3
// - Google Cloud Storage
// - Azure Blob Storage
// - Backblaze B2
// - DigitalOcean Spaces
// - Fine Uploader
// - DropzoneJS
// - Uppy
// - Plupload
// - Wasabi
// - MinIO
// - Filecoin
// - IPFS
// - Arcweave

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::EncodingType;
use aws_sdk_s3::{Client as S3Client, Error as S3Error};
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};

use async_recursion::async_recursion;
use aws_types::region::Region;
use serde_json::Value;
use thiserror::Error;
use urlencoding::decode;

#[derive(Error, Debug)]
pub enum FileTransferError {
    #[error("Network error: {0}")]
    NetworkError(#[from] ReqwestError),
    #[error("Invalid header value")]
    InvalidHeaderValue,
    #[error("Other error: {0}")]
    Other(String),
}

pub type FileDestinationBucket = String;

#[derive(Clone, Debug)]
pub enum FileDestination {
    S3(S3Client, FileDestinationBucket),
    R2(S3Client, FileDestinationBucket),
    Http { url: String, auth_headers: Value },
}

pub async fn upload_file(
    data: Vec<u8>,
    path: &str,
    filename: &str,
    destination: FileDestination,
) -> Result<(), FileTransferError> {
    match destination {
        FileDestination::S3(client, bucket) | FileDestination::R2(client, bucket) => {
            let key = format!("{}/{}", path, filename);
            client
                .put_object()
                .bucket(&bucket)
                .key(&key)
                .body(data.into())
                .content_type("application/octet-stream")
                .send()
                .await
                .map_err(|sdk_error| FileTransferError::Other(format!("S3 error: {:?}", sdk_error)))?;
        }
        FileDestination::Http { url, auth_headers } => {
            let client = Client::new();
            let full_url = format!("{}/{}", url, filename);
            let mut request_builder = client.post(full_url).body(data);

            if let Some(headers) = auth_headers.as_object() {
                for (key, value) in headers {
                    if let Some(header_value) = value.as_str() {
                        request_builder = request_builder.header(key, header_value);
                    } else {
                        return Err(FileTransferError::InvalidHeaderValue);
                    }
                }
            }

            request_builder.send().await.map_err(FileTransferError::from)?;
        }
    }
    Ok(())
}

pub async fn download_file(
    path: &str,
    filename: &str,
    destination: FileDestination,
) -> Result<Vec<u8>, FileTransferError> {
    match destination {
        FileDestination::S3(client, bucket) | FileDestination::R2(client, bucket) => {
            let key = format!("{}/{}", path, filename);
            let result = client.get_object().bucket(&bucket).key(&key).send().await;

            match result {
                Ok(output) => {
                    let stream = output.body.collect().await;
                    match stream {
                        Ok(bytes) => Ok(bytes.into_bytes().to_vec()),
                        Err(_) => Err(FileTransferError::Other("Failed to download file from S3".to_string())),
                    }
                }
                Err(sdk_error) => Err(FileTransferError::Other(format!("S3 error: {:?}", sdk_error))),
            }
        }
        FileDestination::Http { url, auth_headers } => {
            let client = Client::new();
            let full_url = format!("{}/{}", url, filename);
            let mut request_builder = client.get(full_url);

            if let Some(headers) = auth_headers.as_object() {
                for (key, value) in headers {
                    if let Some(header_value) = value.as_str() {
                        request_builder = request_builder.header(key, header_value);
                    } else {
                        return Err(FileTransferError::InvalidHeaderValue);
                    }
                }
            }

            let response = request_builder.send().await.map_err(FileTransferError::from)?;
            if response.status().is_success() {
                let bytes = response.bytes().await.map_err(FileTransferError::from)?;
                Ok(bytes.to_vec())
            } else {
                Err(FileTransferError::Other(format!(
                    "Failed to download file: HTTP {}",
                    response.status()
                )))
            }
        }
    }
}

#[async_recursion]
pub async fn list_folder_contents(
    destination: &FileDestination,
    folder_path: &str,
) -> Result<Vec<String>, FileTransferError> {
    #[derive(Serialize, Deserialize, Debug)]
    pub struct FileDestinationPath {
        pub path: String,
        pub is_folder: bool,
    }

    match destination {
        FileDestination::S3(client, bucket) | FileDestination::R2(client, bucket) => {
            let mut folder_contents = Vec::new();
            let mut continuation_token: Option<String> = None;

            loop {
                let mut request_builder = client
                    .list_objects_v2()
                    .bucket(bucket)
                    .prefix(folder_path)
                    .encoding_type(EncodingType::Url);

                if let Some(token) = &continuation_token {
                    request_builder = request_builder.continuation_token(token);
                }

                let response = request_builder.send().await.map_err(|sdk_error| {
                    FileTransferError::Other(format!("Failed to list folder contents: {:?}", sdk_error))
                })?;

                // Handle files and directories
                if let Some(contents) = response.clone().contents {
                    for object in contents {
                        if let Some(key) = object.key {
                            let decoded_key = decode(&key).unwrap_or_default().to_string();
                            let is_folder = decoded_key.ends_with('/');
                            let clean_path = if is_folder {
                                decoded_key.trim_end_matches('/')
                            } else {
                                &decoded_key
                            };
                            folder_contents.push(FileDestinationPath {
                                path: clean_path.to_string(),
                                is_folder,
                            });
                        }
                    }
                }

                if response.is_truncated().unwrap_or_default() {
                    continuation_token = response.next_continuation_token().map(|s| s.to_string());
                } else {
                    break;
                }
            }

            // Optionally, recursively list contents of subdirectories
            let mut all_paths = Vec::new();
            let mut i = 0;
            while i < folder_contents.len() {
                let item = &folder_contents[i];
                all_paths.push(item.path.clone());
                if item.is_folder {
                    let subfolder_contents = list_folder_contents(destination, &item.path).await?;
                    all_paths.extend(subfolder_contents);
                }
                i += 1;
            }

            Ok(all_paths)
        }
        FileDestination::Http { .. } => Err(FileTransferError::Other(
            "Listing folder contents is not supported for HTTP destinations.".to_string(),
        )),
    }
}

// Function to generate a temporary shareable link for a file for 1 hour
pub async fn generate_temporary_shareable_link(
    path: &str,
    filename: &str,
    destination: &FileDestination,
) -> Result<String, FileTransferError> {
    match destination {
        FileDestination::S3(client, bucket) => {
            let key = format!("{}/{}", path, filename);
            let presigning_config = PresigningConfig::builder() // Remove 'crate::presigning::'
                .expires_in(std::time::Duration::from_secs(3600))
                .build()
                .map_err(|e| FileTransferError::Other(format!("Presigning config error: {:?}", e)))?;

            let presigned_req = client
                .get_object()
                .bucket(bucket)
                .key(&key)
                .response_content_type("application/octet-stream")
                .presigned(presigning_config)
                .await
                .map_err(|e| FileTransferError::Other(format!("S3 presigned URL error: {:?}", e)))?;

            Ok(presigned_req.uri().to_string())
        }
        FileDestination::R2(client, bucket) => {
            // Similar to S3, assuming R2 supports the same presigned URL generation
            let key = format!("{}/{}", path, filename);
            let presigning_config = PresigningConfig::builder() // Remove 'crate::presigning::'
                .expires_in(std::time::Duration::from_secs(3600))
                .build()
                .map_err(|e| FileTransferError::Other(format!("Presigning config error: {:?}", e)))?;

            let presigned_req = client
                .get_object()
                .bucket(bucket)
                .key(&key)
                .response_content_type("application/octet-stream")
                .presigned(presigning_config)
                .await
                .map_err(|e| FileTransferError::Other(format!("R2 presigned URL error: {:?}", e)))?;

            Ok(presigned_req.uri().to_string())
        }
        FileDestination::Http { url, .. } => {
            // For HTTP, we might need to handle this differently as HTTP servers do not typically support presigned URLs
            // This would depend on the specific server's capabilities or additional server-side logic to handle temporary links
            Err(FileTransferError::Other(
                "HTTP destination does not support presigned URLs".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_upload_to_r2() -> Result<(), Box<dyn std::error::Error>> {
        // Generate a unique file name
        let file_name = format!("test_file_{}.txt", Uuid::new_v4());
        let file_contents = b"Hello, R2!"; // Dummy file contents
        let file_path = "test_path_a/test_path_b";

        // Set environment variables for AWS credentials
        // TODO: Remove this and also reset Cloudflare's token!!!
        std::env::set_var("AWS_ACCESS_KEY_ID", "462e168d6b11100c5fe01c39410f3c5f");
        std::env::set_var(
            "AWS_SECRET_ACCESS_KEY",
            "e0e4e19c3b9ad5e51018a255aa08ca098c9e095e737f9d5193d9c88f9492c845",
        );

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";
        let config = aws_config::load_from_env().await;
        let s3_config = config
            .into_builder()
            .endpoint_url(cloudflare_kv_uri)
            .region(Region::new("us-east-1")) // Cloudflare R2 uses 'us-east-1' as a placeholder
            .build();

        let client = S3Client::new(&s3_config);

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let destination = FileDestination::R2(client, bucket_name.to_string());

        // Call the upload function
        let upload_result = upload_file(file_contents.to_vec(), file_path, &file_name, destination.clone()).await;
        eprintln!("{:?}", upload_result);

        // Assert that the upload was successful
        assert!(upload_result.is_ok());

        // Optionally, you can check if the file exists at the URL (requires additional GET request logic)
        let download_result = download_file(file_path, &file_name, destination).await;
        assert!(download_result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_download_and_link_r2() -> Result<(), Box<dyn std::error::Error>> {
        // Setup test data
        let file_name = format!("test_file_{}.txt", Uuid::new_v4());
        let file_contents = b"Hello, R2!"; // Dummy file contents
        let file_path = "test_path_a/test_path_b";

        // Set environment variables for AWS credentials
        std::env::set_var("AWS_ACCESS_KEY_ID", "462e168d6b11100c5fe01c39410f3c5f");
        std::env::set_var(
            "AWS_SECRET_ACCESS_KEY",
            "e0e4e19c3b9ad5e51018a255aa08ca098c9e095e737f9d5193d9c88f9492c845",
        );

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";
        let config = aws_config::load_from_env().await;
        let s3_config = config
            .into_builder()
            .endpoint_url(cloudflare_kv_uri)
            .region(Region::new("us-east-1")) // Cloudflare R2 uses 'us-east-1' as a placeholder
            .build();

        let client = S3Client::new(&s3_config);

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let destination = FileDestination::R2(client, bucket_name.to_string());

        // Upload the file
        let upload_result = upload_file(file_contents.to_vec(), file_path, &file_name, destination.clone()).await;
        assert!(upload_result.is_ok(), "Upload failed: {:?}", upload_result);

        // Generate a temporary shareable link
        let link_result = generate_temporary_shareable_link(file_path, &file_name, &destination).await;
        assert!(link_result.is_ok(), "Failed to generate link: {:?}", link_result);
        let link = link_result.unwrap();

        // Download the file using the generated link (simulating HTTP GET request)
        let client = reqwest::Client::new();
        let download_response = client.get(&link).send().await?;
        assert!(
            download_response.status().is_success(),
            "Download failed: HTTP {}",
            download_response.status()
        );

        let downloaded_bytes = download_response.bytes().await?;
        assert_eq!(
            downloaded_bytes.as_ref(),
            file_contents,
            "Downloaded content does not match uploaded content"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_folder_contents_r2() -> Result<(), Box<dyn std::error::Error>> {
        // Set environment variables for AWS credentials
        std::env::set_var("AWS_ACCESS_KEY_ID", "462e168d6b11100c5fe01c39410f3c5f");
        std::env::set_var(
            "AWS_SECRET_ACCESS_KEY",
            "e0e4e19c3b9ad5e51018a255aa08ca098c9e095e737f9d5193d9c88f9492c845",
        );

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";
        let config = aws_config::load_from_env().await;
        let s3_config = config
            .into_builder()
            .endpoint_url(cloudflare_kv_uri)
            .region(Region::new("us-east-1")) // Cloudflare R2 uses 'us-east-1' as a placeholder
            .build();

        let client = S3Client::new(&s3_config);

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let destination = FileDestination::R2(client, bucket_name.to_string());

        // Folder path to list contents
        let folder_path = "";

        // Call the list_folder_contents function
        let list_result = list_folder_contents(&destination, folder_path).await;
        println!("results: {:?}", list_result);

        // Assert that the list operation was successful
        assert!(list_result.is_ok());

        // Optionally, check the contents of the list
        if let Ok(contents) = list_result {
            assert!(!contents.is_empty(), "The folder contents should not be empty");
            println!("Folder contents: {:?}", contents);
        }

        Ok(())
    }
}
