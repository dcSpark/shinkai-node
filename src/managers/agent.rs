use super::agent_serialization::SerializedAgent;
use super::providers::openai::OpenAI;
use super::providers::sleep_api::SleepAPI;
use crate::{
    managers::providers::Provider,
    schemas::message_schemas::{JobPreMessage, JobRecipient},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String, // user-specified name (sub-identity)
    pub job_manager_sender: mpsc::Sender<Vec<JobPreMessage>>,
    pub agent_receiver: Arc<Mutex<mpsc::Receiver<String>>>,
    pub client: Client,
    pub perform_locally: bool,        // flag to perform computation locally or not
    pub external_url: Option<String>, // external API URL
    pub api_key: Option<String>,
    pub model: AgentAPIModel,
    pub toolkit_permissions: Vec<String>, // list of toolkits the agent has access to
    pub storage_bucket_permissions: Vec<String>, // list of storage buckets the agent has access to
    pub allowed_message_senders: Vec<String>, // list of sub-identities allowed to message the agent
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
        job_manager_sender: mpsc::Sender<Vec<JobPreMessage>>,
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

    pub async fn call_external_api(
        &self,
        content: &str,
        context: Vec<String>,
    ) -> Result<Vec<JobPreMessage>, AgentError> {
        match &self.model {
            AgentAPIModel::OpenAI(openai) => {
                openai
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), content, context)
                    .await
            }
            AgentAPIModel::Sleep(sleep_api) => {
                sleep_api
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), content, context)
                    .await
            }
        }
    }

    pub async fn process_locally(&self, content: String, context: Vec<String>) {
        // Here we run our GPU-intensive task on a separate thread
        let handle = tokio::task::spawn_blocking(move || {
            // perform GPU-intensive work
            vec![JobPreMessage {
                tool_calls: Vec::new(), // You might want to replace this with actual values
                content: "Updated response!".to_string(),
                recipient: JobRecipient::SelfNode, // This is a placeholder. You should replace this with the actual recipient.
            }]
        });

        let result = handle.await;
        match result {
            Ok(response) => {
                // create ShinkaiMessage based on result and send to AgentManager
                let _ = self.job_manager_sender.send(response).await;
            }
            Err(e) => eprintln!("Error in processing message: {:?}", e),
        }
    }

    // TODO: add context as input which should be a Vec<String>
    pub async fn execute(&self, content: String, context: Vec<String>) {
        if self.perform_locally {
            // No need to spawn a new task here
            self.process_locally(content.clone(), context).await;
        } else {
            // Call external API
            let response = self.call_external_api(&content.clone(), context).await; // Assuming the content doesn't change
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

impl Agent {
    pub fn from_serialized_agent(serialized_agent: SerializedAgent, sender: mpsc::Sender<Vec<JobPreMessage>>) -> Self {
        Self::new(
            serialized_agent.id,
            serialized_agent.name,
            sender,
            serialized_agent.perform_locally,
            serialized_agent.external_url,
            serialized_agent.api_key,
            serialized_agent.model,
            serialized_agent.toolkit_permissions,
            serialized_agent.storage_bucket_permissions,
            serialized_agent.allowed_message_senders,
        )
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
        let context = vec![String::from("context1"), String::from("context2")];

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
            agent.execute("Test".to_string(), context).await;
        });

        let val = tokio::time::timeout(std::time::Duration::from_millis(600), rx.recv()).await;
        let expected_resp = JobPreMessage {
            tool_calls: Vec::new(),
            content: "Updated response!".to_string(),
            recipient: JobRecipient::SelfNode,
        };

        match val {
            Ok(Some(response)) => assert_eq!(response.first().unwrap(), &expected_resp),
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

        let context = vec![String::from("context1"), String::from("context2")];
        let (tx, _rx) = mpsc::channel(1);
        let openai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
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

        let response = agent.call_external_api("Hello!", context).await;
        let expected_resp = JobPreMessage {
            tool_calls: Vec::new(),
            content: "\n\nHello there, how may I assist you today?".to_string(),
            recipient: JobRecipient::SelfNode,
        };
        match response {
            Ok(res) => assert_eq!(res.first().unwrap(), &expected_resp),
            Err(e) => panic!("Error when calling API: {}", e),
        }
    }
}
