use async_trait::async_trait;

use crate::{
    prompts::{STAGE_1_PROMPT, STAGE_2_PROMPT},
    sheet_compressor::IndexDictionary,
};

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
        let stage_1_prompt = STAGE_1_PROMPT.replace("{table_input}", &dictionary.to_string());
        let stage_1_question = format!("{}\n QUESTION: {}", stage_1_prompt, question);
        let stage_1_answer = client.chat(&stage_1_question).await?;

        let stage_2_prompt = STAGE_2_PROMPT.replace("{table_input}", &stage_1_answer);
        let stage_2_question = format!("{}\n QUESTION: {}", stage_2_prompt, question);
        let stage_2_answer = client.chat(&stage_2_question).await?;

        Ok(stage_2_answer)
    }
}
