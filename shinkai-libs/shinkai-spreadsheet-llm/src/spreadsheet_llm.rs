use async_trait::async_trait;

use crate::{
    prompts::{FIND_TABLE_PROMPT, UNCOMPRESSED_QUESTION_PROMPT},
    sheet_compressor::{CompressedSheet, MarkdownCell},
};

#[async_trait]
pub trait LLMClient {
    async fn chat(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct SpreadsheetLLM {}

impl SpreadsheetLLM {
    pub async fn uncompressed_question(
        client: &(dyn LLMClient),
        question: &str,
        markdown: &Vec<MarkdownCell>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let md_string = markdown
            .iter()
            .map(|cell| format!("{},{}", cell.address, cell.value))
            .collect::<Vec<String>>()
            .join("|");
        let question_prompt = UNCOMPRESSED_QUESTION_PROMPT
            .replace("{table_input}", &md_string)
            .replace("{question}", question);
        let question_answer = client.chat(&question_prompt).await?;

        Ok(question_answer)
    }

    pub async fn find_table(
        client: &(dyn LLMClient),
        question: &str,
        compressed_sheet: &CompressedSheet,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let prompt = FIND_TABLE_PROMPT
            .replace("{table_input}", &compressed_sheet.dictionary.to_string())
            .replace("{question}", question);
        let answer = client.chat(&prompt).await?;

        Ok(answer)
    }

    // pub async fn compressed_question(
    //     client: &(dyn LLMClient),
    //     question: &str,
    //     dictionary: &IndexDictionary,
    // ) -> Result<String, Box<dyn std::error::Error>> {
    //     let question_prompt = COMPRESSED_QUESTION_PROMPT
    //         .replace("{table_input}", &dictionary.to_string())
    //         .replace("{question}", question);
    //     let question_answer = client.chat(&question_prompt).await?;

    //     Ok(question_answer)
    // }
}
