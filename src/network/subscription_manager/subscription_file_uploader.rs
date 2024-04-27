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

use cloudflare_r2_rs::r2::R2Manager;
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FileTransferError {
    #[error("Network error: {0}")]
    NetworkError(#[from] ReqwestError),
    #[error("Invalid header value")]
    InvalidHeaderValue,
    #[error("Other error: {0}")]
    Other(String),
}

// For later (implementation of the serialization and deserialization of the FileDestination enum)
// #[derive(Debug, Clone)]
// pub struct R2Manager {
//      bucket_name: String,
//      client: Arc<Client>
// }

// impl R2Manager {
//      /// Creates a new instance of R2Manager. The region is set to us-east-1 which aliases
//      /// to auto. Read more here <https://developers.cloudflare.com/r2/api/s3/api/>.
//      pub async fn new(
//           bucket_name: &str,
//           cloudflare_kv_uri: &str, 
//           cloudflare_kv_client_id: &str,
//           cloudflare_kv_secret: &str
//      ) -> R2Manager {

#[derive(Clone, Debug)]
pub enum FileDestination {
    R2(R2Manager),
    Http {
        url: String,
        auth_headers: Value,
    },
}

pub async fn upload_file(data: Vec<u8>, path: &str, filename: &str, destination: FileDestination) -> Result<(), FileTransferError> {
    match destination {
        FileDestination::R2(manager) => {
            manager.upload(filename, &data, None, Some("application/octet-stream")).await;
            // Since the upload method handles errors internally and logs them, we do not need to handle them here.
            // If you need to handle errors, you might need to modify the R2Manager to return a Result type.
        },
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

pub async fn download_file(path: &str, filename: &str, destination: FileDestination) -> Result<Vec<u8>, FileTransferError> {
    match destination {
        FileDestination::R2(manager) => {
            if let Some(bytes) = manager.get(filename).await {
                Ok(bytes)
            } else {
                Err(FileTransferError::Other("Failed to download file".to_string()))
            }
        },
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
                Err(FileTransferError::Other(format!("Failed to download file: HTTP {}", response.status())))
            }
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
        let folder_name = "test_folder";
        let file_name = format!("{}/test_file_{}.txt", folder_name, Uuid::new_v4());
        let file_contents = b"Hello, R2!";  // Dummy file contents
        let file_path = "test_path";

        // Setup the R2Manager
        let r2_manager = R2Manager::new(
            "shinkai-streamer",
            "https://54bf1bf573b3e6471e574cc4d318db64.r2.cloudflarestorage.com",
            "462e168d6b11100c5fe01c39410f3c5f",
            "e0e4e19c3b9ad5e51018a255aa08ca098c9e095e737f9d5193d9c88f9492c845"
        ).await;

        // Setup the destination
        let destination = FileDestination::R2(r2_manager);

        // Call the upload function
        let upload_result = upload_file(file_contents.to_vec(), file_path, &file_name, destination.clone()).await;

        // Assert that the upload was successful
        assert!(upload_result.is_ok());

        // Optionally, you can check if the file exists at the URL (requires additional GET request logic)
        let download_result = download_file(file_path, &file_name, destination).await;
        assert!(download_result.is_ok());

        Ok(())
    }
}