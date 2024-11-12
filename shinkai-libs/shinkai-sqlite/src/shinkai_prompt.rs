use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiPrompt {
    pub rowid: Option<i64>,
    pub name: String,
    pub is_system: bool,
    pub is_enabled: bool,
    pub version: String,
    pub prompt: String,
    pub is_favorite: bool,
}
