use crate::{shinkai_message::shinkai_message_extension::ShinkaiMessageWrapper, shinkai_message_proto::ShinkaiMessage};
use reqwest::Client;
use std::fmt;
use std::{error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};

#[derive(Clone)]
pub struct Agent {
    id: String,
    name: String, // user-specified name (sub-identity)
    job_manager_sender: mpsc::Sender<ShinkaiMessage>,
    agent_receiver: Arc<Mutex<mpsc::Receiver<ShinkaiMessage>>>,
    client: Client,
    perform_locally: bool,                   // flag to perform computation locally or not
    external_url: Option<String>,            // external API URL
    toolkit_permissions: Vec<String>,        // list of toolkits the agent has access to
    storage_bucket_permissions: Vec<String>, // list of storage buckets the agent has access to
    allowed_message_senders: Vec<String>,    // list of sub-identities allowed to message the agent
}

impl Agent {
    pub fn new(
        id: String,
        name: String,
        job_manager_sender: mpsc::Sender<ShinkaiMessage>,
        perform_locally: bool,
        external_url: Option<String>,
        toolkit_permissions: Vec<String>,
        storage_bucket_permissions: Vec<String>,
        allowed_message_senders: Vec<String>,
    ) -> Self {
        let client = Client::new();
        let (agent_sender, agent_receiver) = mpsc::channel(1);
        let agent_receiver = Arc::new(Mutex::new(agent_receiver)); // wrap the receiver
        Self {
            id,
            name,
            job_manager_sender,
            agent_receiver,
            client,
            perform_locally,
            external_url,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
        }
    }

    pub async fn call_external_api(&self) -> Result<ShinkaiMessage, AgentError> {
        if let Some(ref url) = self.external_url {
            let res = self.client.get(url).send().await?;
            let data: ShinkaiMessageWrapper = res.json().await.map_err(AgentError::ReqwestError)?;
            Ok(data.into())
        } else {
            Err(AgentError::UrlNotSet)
        }
    }

    pub async fn process_message(&self, message: ShinkaiMessage) {
        // Here we run our GPU-intensive task on a separate thread
        let handle = tokio::task::spawn_blocking(move || {
            // perform GPU-intensive work
        });

        let result = handle.await;
        match result {
            Ok(_) => (),
            Err(e) => eprintln!("Error in processing message: {:?}", e),
        }
    }

    // pub async fn send_message(&self, msg: ShinkaiMessage) {
    //     // Check if the sender is in the list of allowed senders
    //     if self.allowed_message_senders.contains(&msg.get_sender()) {
    //         let _ = self.sender.send(msg).await;
    //     } else {
    //         eprintln!("Unauthorized message sender!");
    //     }
    // }

    pub async fn start(&self) {
        loop {
            if self.perform_locally {
                // Extract message from queue and process it locally
                let message = self.agent_receiver.lock().await.recv().await;
                match message {
                    Some(message) => {
                        self.process_message(message).await;
                    }
                    None => {
                        eprintln!("Error when getting message from the queue.");
                    }
                }
            } else {
                // Call external API
                let response = self.call_external_api().await;
                match response {
                    Ok(message) => {
                        self.process_message(message).await;
                    }
                    Err(e) => eprintln!("Error when calling API: {}", e),
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // sleep for 5 seconds
        }
    }
}

pub enum AgentError {
    UrlNotSet,
    ReqwestError(reqwest::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => write!(f, "URL is not set"),
            AgentError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
        }
    }
}

impl fmt::Debug for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => f.debug_tuple("UrlNotSet").finish(),
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
    use crate::shinkai_message::{
        encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::unsafe_deterministic_signature_keypair,
    };

    use super::*;
    use mockito::Server;
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_agent_creation() {
        let (tx, _rx) = mpsc::channel(1);
        let agent = Agent::new(
            "1".to_string(),
            "Agent".to_string(),
            tx,
            true,
            Some("http://localhost:8000".to_string()),
            vec!["tk1".to_string(), "tk2".to_string()],
            vec!["sb1".to_string(), "sb2".to_string()],
            vec!["allowed1".to_string(), "allowed2".to_string()],
        );

        assert_eq!(agent.id, "1");
        assert_eq!(agent.name, "Agent");
        assert_eq!(agent.perform_locally, true);
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
    }

    // #[tokio::test]
    // async fn test_shinkai_message_with_mockito() {
    //     let mut server = mockito::Server::new_async().await;
    //     let m1 = server.mock("GET", "/a").with_body("aaa").create_async().await;

    //     let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
    //     let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    //     let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    //     let recipient = "@@other_node.shinkai".to_string();
    //     let sender = "@@my_node.shinkai".to_string();

    //     let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
    //         .body(server.url().to_string()) // Here we're using the mocked URL as the message body
    //         .no_body_encryption()
    //         .message_schema_type("schema type".to_string())
    //         .internal_metadata(
    //             "".to_string(),
    //             "".to_string(),
    //             "".to_string(),
    //             EncryptionMethod::DiffieHellmanChaChaPoly1305,
    //         )
    //         .external_metadata(recipient, sender.clone())
    //         .build();

    //     assert!(message_result.is_ok());
    //     let message = message_result.unwrap();

    //     let url = message.body.clone().unwrap().content;
    //     println!("URL: {:?}", url); // Debug print

    //     // Perform an HTTP request using the message body as the URL
    //     let response = reqwest::get(message.body.unwrap().content).await.unwrap();
    //     assert_eq!(response.status().as_u16(), 200);
    //     let response_body = response.text().await.unwrap();
    //     assert_eq!(response_body, "aaa");

    //     m1.assert_async().await;
    // }
}
