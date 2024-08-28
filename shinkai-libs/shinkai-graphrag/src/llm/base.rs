use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct BaseLLMCallback {
    pub response: Vec<String>,
}

impl BaseLLMCallback {
    pub fn on_llm_new_token(&mut self, token: &str) {
        self.response.push(token.to_string());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    String(String),
    Strings(Vec<String>),
    Dictionary(Vec<HashMap<String, String>>),
}

#[derive(Debug, Clone)]
pub struct LLMParams {
    pub max_tokens: u32,
    pub temperature: f32,
}

#[async_trait]
pub trait BaseLLM {
    async fn agenerate(
        &self,
        messages: MessageType,
        streaming: bool,
        callbacks: Option<Vec<BaseLLMCallback>>,
        llm_params: LLMParams,
        search_phase: Option<GlobalSearchPhase>,
    ) -> anyhow::Result<String>;
}

#[async_trait]
pub trait BaseTextEmbedding {
    async fn aembed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

pub enum GlobalSearchPhase {
    Map,
    Reduce,
}
