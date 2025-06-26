use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schemas::shinkai_tool_offering::ShinkaiToolOffering;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentNetworkOfferingRequest {
    pub agent_identity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentNetworkOfferingResponse {
    pub offerings: Option<Vec<ShinkaiToolOffering>>,
    pub last_updated: Option<DateTime<Utc>>,
}
