use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPrompt {
    pub rowid: Option<i64>,
    pub name: String,
    pub prompt: String,
    pub is_system: bool,
    pub is_enabled: bool,
    pub version: String,
    pub is_favorite: bool,
}
