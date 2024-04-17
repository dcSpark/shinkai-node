use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Default)]
pub struct FileUploadResponse {
    pub name: String,
    pub path: String,
    pub merkle_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileInfo {
    pub merkle_hash: String,
    pub name: String,
    pub source_file_map_last_saved_datetime: String,
    pub distribution_info: Option<DistributionInfo>,
    pub created_datetime: String,
    pub last_written_datetime: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DistributionInfo {
    pub datetime: String,
    pub origin: Option<String>,
}