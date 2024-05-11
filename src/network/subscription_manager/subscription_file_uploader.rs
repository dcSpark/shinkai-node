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

use std::env;

use aws_config::credential_process;
use aws_config::meta::region::RegionProviderChain;
use aws_config::profile::credentials;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::EncodingType;
use aws_sdk_s3::{Client as S3Client, Error as S3Error};
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};

use async_recursion::async_recursion;
use aws_types::region::Region;
use serde_json::{json, Error as JsonError, Value};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::FileDestinationCredentials;
use thiserror::Error;
use urlencoding::decode;

#[derive(Error, Debug)]
pub enum FileDestinationError {
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] JsonError),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unknown type field")]
    UnknownTypeField,
}

#[derive(Error, Debug)]
pub enum FileTransferError {
    #[error("Network error: {0}")]
    NetworkError(#[from] ReqwestError),
    #[error("Invalid header value")]
    InvalidHeaderValue,
    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Clone, Debug)]
pub enum FileDestination {
    S3(S3Client, FileDestinationCredentials),
    R2(S3Client, FileDestinationCredentials),
    Http { url: String, auth_headers: Value },
}

impl FileDestination {
    pub fn to_json(&self) -> Value {
        match self {
            FileDestination::S3(_, credentials) => {
                json!({
                    "type": "S3",
                    "credentials": {
                        "access_key_id": credentials.access_key_id,
                        "secret_access_key": credentials.secret_access_key,
                        "endpoint_uri": credentials.endpoint_uri,
                        "bucket": credentials.bucket
                    }
                })
            }
            FileDestination::R2(_, credentials) => {
                json!({
                    "type": "R2",
                    "credentials": {
                        "access_key_id": credentials.access_key_id,
                        "secret_access_key": credentials.secret_access_key,
                        "endpoint_uri": credentials.endpoint_uri,
                        "bucket": credentials.bucket
                    }
                })
            }
            FileDestination::Http { url, auth_headers } => {
                json!({
                    "type": "Http",
                    "url": url,
                    "auth_headers": auth_headers
                })
            }
        }
    }

    pub async fn from_credentials(credentials: FileDestinationCredentials) -> Result<Self, FileDestinationError> {
        // Set environment variables for AWS credentials
        env::set_var("AWS_ACCESS_KEY_ID", &credentials.access_key_id);
        env::set_var("AWS_SECRET_ACCESS_KEY", &credentials.secret_access_key);

        // Setup the S3Client using environment configuration
        let config = aws_config::load_from_env().await;

        let s3_config = config
            .into_builder()
            .endpoint_url(&credentials.endpoint_uri)
            .region(Region::new("us-east-1")) // Placeholder region
            .build();

        let client = S3Client::new(&s3_config);

        Ok(FileDestination::S3(client, credentials))
    }

    pub async fn from_json(value: &Value) -> Result<Self, FileDestinationError> {
        let type_field = value
            .get("type")
            .ok_or(FileDestinationError::InvalidInput("Missing type field".to_string()))?
            .as_str()
            .ok_or(FileDestinationError::InvalidInput(
                "Type field should be a string".to_string(),
            ))?;

        match type_field {
            "S3" | "R2" => {
                let credentials = value
                    .get("credentials")
                    .ok_or(FileDestinationError::InvalidInput("Missing credentials".to_string()))?;
                let access_key_id = credentials
                    .get("access_key_id")
                    .ok_or(FileDestinationError::InvalidInput("Missing access_key_id".to_string()))?
                    .as_str()
                    .ok_or(FileDestinationError::InvalidInput(
                        "access_key_id should be a string".to_string(),
                    ))?
                    .to_string();
                let secret_access_key = credentials
                    .get("secret_access_key")
                    .ok_or(FileDestinationError::InvalidInput(
                        "Missing secret_access_key".to_string(),
                    ))?
                    .as_str()
                    .ok_or(FileDestinationError::InvalidInput(
                        "secret_access_key should be a string".to_string(),
                    ))?
                    .to_string();
                let endpoint_uri = credentials
                    .get("endpoint_uri")
                    .ok_or(FileDestinationError::InvalidInput("Missing endpoint_uri".to_string()))?
                    .as_str()
                    .ok_or(FileDestinationError::InvalidInput(
                        "endpoint_uri should be a string".to_string(),
                    ))?
                    .to_string();
                let bucket = credentials
                    .get("bucket")
                    .ok_or(FileDestinationError::InvalidInput("Missing bucket".to_string()))?
                    .as_str()
                    .ok_or(FileDestinationError::InvalidInput(
                        "bucket should be a string".to_string(),
                    ))?
                    .to_string();

                // Set environment variables for AWS credentials
                env::set_var("AWS_ACCESS_KEY_ID", &access_key_id);
                env::set_var("AWS_SECRET_ACCESS_KEY", &secret_access_key);

                // Setup the S3Client using environment configuration
                let config = aws_config::load_from_env().await;

                let s3_config = config
                    .into_builder()
                    .endpoint_url(&endpoint_uri)
                    .region(Region::new("us-east-1")) // Placeholder region
                    .build();

                let client = S3Client::new(&s3_config);

                match type_field {
                    "S3" => Ok(FileDestination::S3(
                        client,
                        FileDestinationCredentials {
                            access_key_id,
                            secret_access_key,
                            endpoint_uri,
                            bucket,
                        },
                    )),
                    "R2" => Ok(FileDestination::R2(
                        client,
                        FileDestinationCredentials {
                            access_key_id,
                            secret_access_key,
                            endpoint_uri,
                            bucket,
                        },
                    )),
                    _ => Err(FileDestinationError::UnknownTypeField),
                }
            }
            "Http" => {
                let url = value
                    .get("url")
                    .ok_or(FileDestinationError::InvalidInput("Missing url field".to_string()))?
                    .as_str()
                    .ok_or(FileDestinationError::InvalidInput(
                        "Url field should be a string".to_string(),
                    ))?
                    .to_string();
                let auth_headers = value
                    .get("auth_headers")
                    .ok_or(FileDestinationError::InvalidInput("Missing auth_headers".to_string()))?
                    .clone();

                Ok(FileDestination::Http { url, auth_headers })
            }
            _ => Err(FileDestinationError::UnknownTypeField),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileDestinationPath {
    pub path: String,
    pub is_folder: bool,
}

pub async fn upload_file(
    data: Vec<u8>,
    path: &str,
    filename: &str,
    destination: FileDestination,
) -> Result<(), FileTransferError> {
    match destination {
        FileDestination::S3(client, credentials) | FileDestination::R2(client, credentials) => {
            let key = format!("{}/{}", path, filename);
            client
                .put_object()
                .bucket(&credentials.bucket)
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
        FileDestination::S3(client, credentials) | FileDestination::R2(client, credentials) => {
            let key = format!("{}/{}", path, filename);
            let result = client.get_object().bucket(&credentials.bucket).key(&key).send().await;

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
) -> Result<Vec<FileDestinationPath>, FileTransferError> {
    match destination {
        FileDestination::S3(client, credentials) | FileDestination::R2(client, credentials) => {
            let mut folder_contents = Vec::new();
            let mut continuation_token: Option<String> = None;

            loop {
                let mut request_builder = client
                    .list_objects_v2()
                    .bucket(credentials.bucket.clone())
                    .prefix(folder_path)
                    .delimiter("/")
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

                // Handle common prefixes (subdirectories)
                if let Some(common_prefixes) = response.clone().common_prefixes {
                    for prefix in common_prefixes {
                        if let Some(prefix_key) = prefix.prefix {
                            let decoded_key = decode(&prefix_key).unwrap_or_default().to_string();
                            let clean_path = decoded_key.trim_end_matches('/');
                            if !clean_path.is_empty() {
                                folder_contents.push(FileDestinationPath {
                                    path: clean_path.to_string(),
                                    is_folder: true,
                                });
                            }
                        }
                    }
                }

                if response.is_truncated().unwrap_or_default() {
                    continuation_token = response.next_continuation_token().map(|s| s.to_string());
                } else {
                    break;
                }
            }
            eprintln!("Folder contents: {:?}", folder_contents);

            // Recursively list contents of subdirectories
            let mut all_contents = Vec::new();
            for item in folder_contents {
                eprintln!("Item: {:?}", item);
                all_contents.push(item.clone());
                if item.is_folder {
                    let subfolder_contents = list_folder_contents(destination, &format!("{}/", item.path)).await?;
                    all_contents.extend(subfolder_contents);
                }
            }

            Ok(all_contents)
        }
        FileDestination::Http { .. } => Err(FileTransferError::Other(
            "Listing folder contents is not supported for HTTP destinations.".to_string(),
        )),
    }
}

/// Generates temporary shareable links for all files in a specified folder.
#[async_recursion]
pub async fn generate_temporary_shareable_links_for_folder(
    folder_path: &str,
    destination: &FileDestination,
) -> Result<Vec<(String, String)>, FileTransferError> {
    let contents = list_folder_contents(destination, folder_path).await?;
    let mut links = Vec::new();

    for item in contents {
        if !item.is_folder {
            match generate_temporary_shareable_link(folder_path, &item.path, destination).await {
                Ok(link) => links.push((item.path, link)),
                Err(e) => return Err(e),
            }
        }
    }
    
    Ok(links)
}

// Function to generate a temporary shareable link for a file for 1 hour
pub async fn generate_temporary_shareable_link(
    path: &str,
    filename: &str,
    destination: &FileDestination,
) -> Result<String, FileTransferError> {
    match destination {
        FileDestination::S3(client, credentials) => {
            let key = format!("{}/{}", path, filename);
            let presigning_config = PresigningConfig::builder() // Remove 'crate::presigning::'
                .expires_in(std::time::Duration::from_secs(3600))
                .build()
                .map_err(|e| FileTransferError::Other(format!("Presigning config error: {:?}", e)))?;

            let presigned_req = client
                .get_object()
                .bucket(credentials.bucket.clone())
                .key(&key)
                .response_content_type("application/octet-stream")
                .presigned(presigning_config)
                .await
                .map_err(|e| FileTransferError::Other(format!("S3 presigned URL error: {:?}", e)))?;

            Ok(presigned_req.uri().to_string())
        }
        FileDestination::R2(client, credentials) => {
            // Similar to S3, assuming R2 supports the same presigned URL generation
            let key = format!("{}/{}", path, filename);
            let presigning_config = PresigningConfig::builder() // Remove 'crate::presigning::'
                .expires_in(std::time::Duration::from_secs(3600))
                .build()
                .map_err(|e| FileTransferError::Other(format!("Presigning config error: {:?}", e)))?;

            let presigned_req = client
                .get_object()
                .bucket(credentials.bucket.clone())
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

pub async fn delete_file_or_folder(destination: &FileDestination, path: &str) -> Result<(), FileTransferError> {
    match destination {
        FileDestination::S3(client, credentials) | FileDestination::R2(client, credentials) => {
            let result = client.delete_object().bucket(credentials.bucket.clone()).key(path).send().await;

            result.map_err(|sdk_error| FileTransferError::Other(format!("S3/R2 delete error: {:?}", sdk_error)))?;
        }
        FileDestination::Http { url, auth_headers } => {
            let client = Client::new();
            let full_url = format!("{}/{}", url, path);
            let mut request_builder = client.delete(full_url);

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
            if !response.status().is_success() {
                return Err(FileTransferError::Other(format!(
                    "Failed to delete file or folder: HTTP {}",
                    response.status()
                )));
            }
        }
    }
    Ok(())
}

/// Deletes all files and folders recursively within a specified folder.
#[async_recursion]
pub async fn delete_all_in_folder(destination: &FileDestination, folder_path: &str) -> Result<(), FileTransferError> {
    let contents = list_folder_contents(destination, folder_path).await?;
    // Start by deleting all files in the current folder
    for item in &contents {
        if !item.is_folder {
            let full_path = format!("{}/{}", folder_path, item.path);
            delete_file_or_folder(destination, &full_path).await?;
        }
    }
    // Then delete subfolders recursively
    for item in contents {
        if item.is_folder {
            let full_path = format!("{}/{}", folder_path, item.path);
            delete_all_in_folder(destination, &full_path).await?;
            // After deleting all contents in the subfolder, delete the subfolder itself
            delete_file_or_folder(destination, &full_path).await?;
        }
    }
    Ok(())
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

        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let credentials = FileDestinationCredentials::new(
            access_key_id,
            secret_access_key,
            cloudflare_kv_uri.to_string(),
            bucket_name.to_string(),
        );
        let destination = FileDestination::from_credentials(credentials).await?;

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

        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let credentials = FileDestinationCredentials::new(
            access_key_id,
            secret_access_key,
            cloudflare_kv_uri.to_string(),
            bucket_name.to_string(),
        );
        let destination = FileDestination::from_credentials(credentials).await?;

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
        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let credentials = FileDestinationCredentials::new(
            access_key_id,
            secret_access_key,
            cloudflare_kv_uri.to_string(),
            bucket_name.to_string(),
        );
        let destination = FileDestination::from_credentials(credentials).await?;

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

    #[tokio::test]
    async fn test_upload_and_delete_file_r2() -> Result<(), Box<dyn std::error::Error>> {
        // Setup test data
        let file_name = format!("test_file_{}.txt", Uuid::new_v4());
        let file_contents = b"Hello, R2!"; // Dummy file contents
        let file_path = "test_path_a/test_path_b";

        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let credentials = FileDestinationCredentials::new(
            access_key_id,
            secret_access_key,
            cloudflare_kv_uri.to_string(),
            bucket_name.to_string(),
        );
        let destination = FileDestination::from_credentials(credentials).await?;

        // Upload the file
        let upload_result = upload_file(file_contents.to_vec(), file_path, &file_name, destination.clone()).await;
        assert!(upload_result.is_ok(), "Upload failed: {:?}", upload_result);

        // Delete the file
        let delete_result = delete_file_or_folder(&destination, &format!("{}/{}", file_path, file_name)).await;
        assert!(delete_result.is_ok(), "Delete failed: {:?}", delete_result);

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_multiple_files_and_delete_all() -> Result<(), Box<dyn std::error::Error>> {
        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");

        // Setup the S3Client for R2 using environment configuration
        let cloudflare_kv_uri = "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com";

        // Setup the destination
        let bucket_name = "shinkai-streamer";
        let credentials = FileDestinationCredentials::new(
            access_key_id,
            secret_access_key,
            cloudflare_kv_uri.to_string(),
            bucket_name.to_string(),
        );
        let destination = FileDestination::from_credentials(credentials).await?;

        // Define main folder and subfolder names
        let main_folder_name = "delete_test_folder";
        let subfolder_name = "delete_test_subfolder";

        // Define file names and contents
        let files = vec![
            ("file1.txt", b"Hello, R2 file1!"),
            ("file2.txt", b"Hello, R2 file2!"),
            ("file3.txt", b"Hello, R2 file3!"),
        ];

        // Upload files to the main folder
        for (file_name, content) in &files {
            let upload_result = upload_file(content.to_vec(), main_folder_name, file_name, destination.clone()).await;
            assert!(
                upload_result.is_ok(),
                "Upload failed for {}: {:?}",
                file_name,
                upload_result
            );
        }
        eprintln!("Files uploaded successfully to main folder");

        // Create a subfolder and upload a file into it
        let folder_file_name = "folder_file.txt";
        let folder_file_content = b"Hello, R2 in subfolder!";
        let upload_result = upload_file(
            folder_file_content.to_vec(),
            &format!("{}/{}", main_folder_name, subfolder_name),
            folder_file_name,
            destination.clone(),
        )
        .await;
        assert!(
            upload_result.is_ok(),
            "Upload failed for file in subfolder: {:?}",
            upload_result
        );

        // List contents before deletion
        eprintln!("Calling folder contents");
        let list_result = list_folder_contents(&destination, main_folder_name).await.unwrap();
        eprintln!("\n\nFolder contents before deletion: {:?}", list_result);

        // Delete all files in the main folder
        for file_path in list_result {
            let delete_result = delete_file_or_folder(&destination, &file_path.path).await;
            assert!(
                delete_result.is_ok(),
                "Delete failed for {}: {:?}",
                file_path.path,
                delete_result
            );
        }

        // List contents after deletion to ensure all files and folders are deleted
        let final_list_result = list_folder_contents(&destination, "delete_test_folder").await.unwrap();
        eprintln!("Folder contents after deletion: {:?}", final_list_result);

        assert!(
            final_list_result.is_empty(),
            "Folder contents should be empty after deletion, but found: {:?}",
            final_list_result
        );

        // // Finally, delete the main folder itself
        let delete_main_folder_result = delete_file_or_folder(&destination, main_folder_name).await;
        assert!(
            delete_main_folder_result.is_ok(),
            "Delete failed for main folder: {:?}",
            delete_main_folder_result
        );

        Ok(())
    }
}
