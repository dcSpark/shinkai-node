use std::path::Path;

use async_channel::Sender;
use chrono::{TimeZone, Utc};
use serde_json::Value;

use shinkai_fs::shinkai_fs_error::ShinkaiFsError;
use shinkai_http_api::node_api_router::APIError;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIVecFsCreateFolder;

#[allow(clippy::too_many_arguments)]
pub async fn create_folder(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    folder_name: &str,
    bearer_token: &str,
) {
    let payload = APIVecFsCreateFolder {
        path: folder_path.to_string(),
        folder_name: folder_name.to_string(),
    };

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::V2ApiVecFSCreateFolder {
            bearer: bearer_token.to_string(),
            payload,
            res: res_sender,
        })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("resp: {:?}", resp);
}

#[allow(clippy::too_many_arguments)]
pub async fn upload_file(
    commands_sender: &Sender<NodeCommand>,
    folder_name: &str,
    file_path: &Path,
    bearer_token: &str,
) {
    eprintln!("file_path: {:?}", file_path);

    // Print current directory
    let current_dir = std::env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);

    // Read file data
    let file_data = std::fs::read(file_path)
        .map_err(|_| ShinkaiFsError::FailedPDFParsing)
        .unwrap();

    // Extract the file name and extension
    let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command using V2ApiUploadFileToFolder
    commands_sender
        .send(NodeCommand::V2ApiUploadFileToFolder {
            bearer: bearer_token.to_string(),
            filename, // Use the extracted filename
            file: file_data,
            path: folder_name.to_string(),
            file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            res: res_sender,
        })
        .await
        .unwrap();

    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file resp to folder: {:?}", resp);
}

#[allow(clippy::too_many_arguments)]
pub async fn upload_file_to_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    file_path: &Path,
    bearer_token: &str,
) {
    eprintln!("file_path: {:?}", file_path);

    // Print current directory
    let current_dir = std::env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);

    // Read file data
    let file_data = std::fs::read(file_path)
        .map_err(|_| ShinkaiFsError::FailedPDFParsing)
        .unwrap();

    // Extract the file name with extension
    let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command using V2ApiUploadFileToJob
    commands_sender
        .send(NodeCommand::V2ApiUploadFileToJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            filename, // Use the extracted filename
            file: file_data,
            file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            res: res_sender,
        })
        .await
        .unwrap();

    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file_to_job resp: {:?}", resp);
}

pub async fn get_folder_name_for_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    bearer_token: &str,
) -> Result<String, APIError> {
    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to get the folder name for the job
    commands_sender
        .send(NodeCommand::V2ApiVecFSGetFolderNameForJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            res: res_sender,
        })
        .await
        .unwrap();

    // Receive and convert the Value to String
    res_receiver
        .recv()
        .await
        .unwrap()
        .map(|value| value.as_str().unwrap_or_default().to_string())
}

pub async fn get_files_for_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    bearer_token: &str,
) -> Result<Value, APIError> {
    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to retrieve files for the job
    commands_sender
        .send(NodeCommand::V2ApiVecFSRetrieveFilesForJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            res: res_sender,
        })
        .await
        .unwrap();

    // Receive and return the files as a JSON value
    res_receiver.recv().await.unwrap()
}
