use crate::managers::providers::{Provider};
use reqwest::Client;
use std::fmt;
use std::{sync::Arc};
use tokio::sync::{mpsc, Mutex};
use super::providers::openai::{OpenAI};
use super::providers::sleep_api::{SleepAPI};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String, // user-specified name (sub-identity)
    pub job_manager_sender: mpsc::Sender<String>,
    pub agent_receiver: Arc<Mutex<mpsc::Receiver<String>>>,
    pub client: Client,
    pub perform_locally: bool,        // flag to perform computation locally or not
    pub external_url: Option<String>, // external API URL
    pub api_key: Option<String>,
    pub model: AgentAPIModel,
    pub toolkit_permissions: Vec<String>,        // list of toolkits the agent has access to
    pub storage_bucket_permissions: Vec<String>, // list of storage buckets the agent has access to
    pub allowed_message_senders: Vec<String>,    // list of sub-identities allowed to message the agent
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentAPIModel {
    OpenAI(OpenAI), 
    Sleep(SleepAPI),
}

impl Agent {
    pub fn new(
        id: String,
        name: String,
        job_manager_sender: mpsc::Sender<String>,
        perform_locally: bool,
        external_url: Option<String>,
        api_key: Option<String>,
        model: AgentAPIModel,
        toolkit_permissions: Vec<String>,
        storage_bucket_permissions: Vec<String>,
        allowed_message_senders: Vec<String>,
    ) -> Self {
        let client = Client::new();
        let (_, agent_receiver) = mpsc::channel(1);
        let agent_receiver = Arc::new(Mutex::new(agent_receiver)); // wrap the receiver
        Self {
            id,
            name,
            job_manager_sender,
            agent_receiver,
            client,
            perform_locally,
            external_url,
            api_key,
            model,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
        }
    }

    pub async fn call_external_api(&self, content: &str) -> Result<String, AgentError> {
        match &self.model {
            AgentAPIModel::OpenAI(openai) => openai.call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), content).await,
            AgentAPIModel::Sleep(sleep_api) => sleep_api.call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), content).await,
        }
    }

    pub async fn process_message(&self, content: String) {
        // Here we run our GPU-intensive task on a separate thread
        let mut response = "";
        let handle = tokio::task::spawn_blocking(move || {
            // perform GPU-intensive work
            response = "Update response!";
        });

        let result = handle.await;
        match result {
            Ok(_) => {
                // create ShinkaiMessage based on result and send to AgentManager
                let _ = self.job_manager_sender.send(response.to_string()).await;
            }
            Err(e) => eprintln!("Error in processing message: {:?}", e),
        }
    }

    pub async fn execute(&self, content: String) {
        loop {
            if self.perform_locally {
                self.process_message(content.clone()).await; // Assuming the content doesn't change
            } else {
                // Call external API
                let response = self.call_external_api(&content.clone()).await; // Assuming the content doesn't change
                match response {
                    Ok(message) => {
                        // Send the message to AgentManager
                        let _ = self.job_manager_sender.send(message).await;
                    }
                    Err(e) => eprintln!("Error when calling API: {}", e),
                }
            }
        }
    }
}

pub enum AgentError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(reqwest::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => write!(f, "URL is not set"),
            AgentError::ApiKeyNotSet => write!(f, "API Key not set"),
            AgentError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
        }
    }
}

impl fmt::Debug for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => f.debug_tuple("UrlNotSet").finish(),
            AgentError::ApiKeyNotSet => f.debug_tuple("ApiKeyNotSet").finish(),
            AgentError::ReqwestError(err) => f.debug_tuple("ReqwestError").field(err).finish(),
        }
    }
}

impl From<reqwest::Error> for AgentError {
    fn from(err: reqwest::Error) -> AgentError {
        AgentError::ReqwestError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_agent_creation() {
        let (tx, mut rx) = mpsc::channel(1);
        let sleep_api = SleepAPI {};
        let agent = Agent::new(
            "1".to_string(),
            "Agent".to_string(),
            tx,
            false,
            Some("http://localhost:8000".to_string()),
            Some("paramparam".to_string()),
            AgentAPIModel::Sleep(sleep_api),
            vec!["tk1".to_string(), "tk2".to_string()],
            vec!["sb1".to_string(), "sb2".to_string()],
            vec!["allowed1".to_string(), "allowed2".to_string()],
        );

        assert_eq!(agent.id, "1");
        assert_eq!(agent.name, "Agent");
        assert_eq!(agent.perform_locally, false);
        assert_eq!(agent.external_url, Some("http://localhost:8000".to_string()));
        assert_eq!(agent.toolkit_permissions, vec!["tk1".to_string(), "tk2".to_string()]);
        assert_eq!(
            agent.storage_bucket_permissions,
            vec!["sb1".to_string(), "sb2".to_string()]
        );
        assert_eq!(
            agent.allowed_message_senders,
            vec!["allowed1".to_string(), "allowed2".to_string()]
        );

        tokio::spawn(async move {
            agent.execute("Test".to_string()).await;
        });

        let val = tokio::time::timeout(std::time::Duration::from_millis(501), rx.recv()).await;
        match val {
            Ok(Some(response)) => assert_eq!(response, "OK"),
            Ok(None) => panic!("Channel is empty"),
            Err(_) => panic!("Timeout exceeded"),
        }
    }

    #[tokio::test]
    async fn test_agent_call_external_api_openai() {
        let mut server = Server::new();
        let _m = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer mockapikey")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "\n\nHello there, how may I assist you today?"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 9,
                    "completion_tokens": 12,
                    "total_tokens": 21
                }
            }"#,
            )
            .create();

        let (tx, _rx) = mpsc::channel(1);
        let openai = OpenAI { model_type: "gpt-3.5-turbo".to_string() };
        let agent = Agent::new(
            "1".to_string(),
            "Agent".to_string(),
            tx,
            false,
            Some(server.url()), // use the url of the mock server
            Some("mockapikey".to_string()),
            AgentAPIModel::OpenAI(openai),
            vec!["tk1".to_string(), "tk2".to_string()],
            vec!["sb1".to_string(), "sb2".to_string()],
            vec!["allowed1".to_string(), "allowed2".to_string()],
        );

        let response = agent.call_external_api("Hello!").await;
        match response {
            Ok(res) => assert_eq!(res, "\n\nHello there, how may I assist you today?"),
            Err(e) => panic!("Error when calling API: {}", e),
        }
    }
}
