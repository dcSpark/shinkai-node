use async_trait::async_trait;

use crate::{prompts::QUESTION_PROMPT, sheet_compressor::IndexDictionary};

#[async_trait]
pub trait LLMClient {
    async fn chat(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct SpreadsheetLLM {}

impl SpreadsheetLLM {
    pub async fn question(
        client: &(dyn LLMClient),
        question: &str,
        dictionary: &IndexDictionary,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let question_prompt = QUESTION_PROMPT
            .replace("{table_input}", &dictionary.to_string())
            .replace("{question}", question);
        let question_answer = client.chat(&question_prompt).await?;

        Ok(question_answer)
    }
}
