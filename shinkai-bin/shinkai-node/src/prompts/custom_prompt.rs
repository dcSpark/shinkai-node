use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPrompt {
    pub name: String,
    pub prompt: String,
    pub is_system: bool,
    pub is_enabled: bool,
    pub version: String,
    pub is_favorite: bool,
    pub embedding: Option<Vec<f32>>,
}

impl CustomPrompt {
    pub fn text_for_embedding(&self) -> String {
        format!("{} {}", self.name, self.prompt)
    }
}
