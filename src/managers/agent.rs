use reqwest::Client;
use std::error::Error;
use tokio::sync::mpsc;
use crate::{shinkai_message_proto::ShinkaiMessage, shinkai_message::shinkai_message_extension::ShinkaiMessageWrapper};

#[derive(Clone)]
pub struct Agent {
    id: String,
    sender: mpsc::Sender<ShinkaiMessage>,
    client: Client,
}

impl Agent {
    pub fn new(id: String, sender: mpsc::Sender<ShinkaiMessage>) -> Self {
        let client = Client::new();
        Self {
            id,
            sender,
            client,
        }
    }

    pub async fn call_external_api(&self, url: &str) -> Result<ShinkaiMessage, reqwest::Error> {
        let res = self.client.get(url).send().await?;
        let data: ShinkaiMessageWrapper = res.json().await?;
        Ok(data.into())
    }
    
    pub async fn send_message(&self, msg: ShinkaiMessage) {
        let _ = self.sender.send(msg).await;
    }

    pub async fn start(&self) {
        loop {
            let response = self.call_external_api("https://example.com").await;
            match response {
                Ok(message) => {
                    self.send_message(message).await;
                },
                Err(e) => eprintln!("Error when calling API: {}", e),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // sleep for 5 seconds
        }
    }
}
