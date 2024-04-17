use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, Default)]
pub struct FileUploadResponse {
    pub name: String,
    pub path: String,
    pub merkle_hash: String,
}