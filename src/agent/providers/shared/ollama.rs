use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct OllamaAPIResponse {
    pub model: String,
    pub created_at: String,
    pub response: Value,
    pub done: bool,
    pub total_duration: i64,
    pub load_duration: i64,
    pub prompt_eval_count: i32,
    pub prompt_eval_duration: i64,
    pub eval_count: i32,
    pub eval_duration: i64,
}