use serde::{Serialize, Deserialize};
use shinkai_message_wasm::schemas::{shinkai_name::ShinkaiName, agents::serialized_agent::SerializedAgent};

use super::agent::{Agent};

// Agent has a few fields that are not serializable, so we need to create a struct that is serializable
// #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
// pub struct SerializedAgent {
//     pub id: String,
//     pub full_identity_name: ShinkaiName,
//     pub perform_locally: bool,
//     pub external_url: Option<String>,
//     pub api_key: Option<String>,
//     pub model: AgentAPIModel,
//     pub toolkit_permissions: Vec<String>,
//     pub storage_bucket_permissions: Vec<String>,
//     pub allowed_message_senders: Vec<String>,
// }

impl From<Agent> for SerializedAgent {
    fn from(agent: Agent) -> Self {
        SerializedAgent {
            id: agent.id,
            full_identity_name: agent.full_identity_name,
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
