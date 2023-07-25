use std::sync::Arc;

use reqwest::Client;
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, Mutex};

use super::agent::{AgentAPIModel, Agent};

// Agent has a few fields that are not serializable, so we need to create a struct that is serializable
#[derive(Serialize, Deserialize)]
struct SerializedAgent {
    id: String,
    name: String,
    perform_locally: bool,
    external_url: Option<String>,
    api_key: Option<String>,
    model: AgentAPIModel,
    toolkit_permissions: Vec<String>,
    storage_bucket_permissions: Vec<String>,
    allowed_message_senders: Vec<String>,
}

impl From<Agent> for SerializedAgent {
    fn from(agent: Agent) -> Self {
        SerializedAgent {
            id: agent.id,
            name: agent.name,
            perform_locally: agent.perform_locally,
            external_url: agent.external_url,
            api_key: agent.api_key,
            model: agent.model,
            toolkit_permissions: agent.toolkit_permissions,
            storage_bucket_permissions: agent.storage_bucket_permissions,
            allowed_message_senders: agent.allowed_message_senders,
        }
    }
}
