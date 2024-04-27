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
use aws_sdk_s3::{Client as S3Client, Error as S3Error};
use aws_types::region::Region;
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

// Need to implement it manually
// /// Generate a temporary link for downloading an entire folder based on the FileDestination.
// pub async fn generate_temporary_download_link(folder_path: &str, destination: &FileDestination) -> Result<String, FileTransferError> {
//     match destination {
//         FileDestination::R2(manager) => {
//             // Assuming R2Manager has a method to generate a temporary link for a folder
//             manager.generate_temporary_link_for_folder(folder_path).await
//         },
//         FileDestination::Http { url, .. } => {
//             // For HTTP, we might need to construct a URL that points to a directory listing or a zipped folder
//             // This is highly dependent on the HTTP server's capabilities and configuration
//             Ok(format!("{}/{}", url, folder_path))
//         },
//     }
// }

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
}
