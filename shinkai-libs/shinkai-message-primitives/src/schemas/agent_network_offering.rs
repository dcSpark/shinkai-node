use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentNetworkOfferingRequest {
    pub agent_identity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentNetworkOfferingResponse {
    pub agent_identity: String,
    pub value: Option<Value>,
    pub last_updated: Option<DateTime<Utc>>,
}
