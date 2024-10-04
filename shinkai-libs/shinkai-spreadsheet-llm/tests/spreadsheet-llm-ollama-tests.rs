use std::io::Cursor;

use async_trait::async_trait;
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shinkai_spreadsheet_llm::{
    sheet_compressor::SheetCompressor,
    spreadsheet_llm::{LLMClient, SpreadsheetLLM},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaChatMessage,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OllamaChatMessage {
    pub role: String,
    pub content: String,
}

pub struct OllamaChat {
    base_url: String,
    model: String,
}

impl OllamaChat {
    pub fn new(base_url: &str, model: &str) -> Self {
        OllamaChat {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl LLMClient for OllamaChat {
    async fn chat(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        let chat_url = format!("{}/api/chat", self.base_url);
        let messages = json!([{
            "role": "user",
            "content": prompt,
        }]);
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });

        let client = reqwest::Client::new();
        let response = client.post(chat_url).json(&payload).send().await?;
        let response = response.json::<OllamaChatResponse>().await?;

        Ok(response.message.content)
    }
}

#[tokio::test]
async fn sheet_compression_test() {
    let csv_data = std::fs::read("../../files/cars.csv").unwrap();

    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .from_reader(Cursor::new(csv_data));

    let sheet_rows: Vec<Vec<String>> = reader
        .records()
        .map(|r| r.unwrap().iter().map(String::from).collect::<Vec<String>>())
        .collect();

    let compressed_sheet = SheetCompressor::compress_sheet(&sheet_rows);

    let ollama_chat = OllamaChat::new("http://localhost:11434", "llama3.2");

    let answer = SpreadsheetLLM::question(
        &ollama_chat,
        "Which car is the most expensive?",
        &compressed_sheet.dictionary,
    )
    .await
    .unwrap();

    println!("Answer: {}", answer);
}
