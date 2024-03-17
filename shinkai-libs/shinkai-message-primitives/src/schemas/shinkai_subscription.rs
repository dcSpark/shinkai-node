use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::shinkai_name::ShinkaiName;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ShinkaiSubscriptionAction {
    Subscribe,
    Unsubscribe,
    UpdateSubscription,
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ShinkaiSubscriptionRequest {
    pub action: ShinkaiSubscriptionAction,
    pub subscription_id: Option<String>,
    pub vector_db_path: Option<String>,
    pub state: Option<String>, 
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ShinkaiSubscription {
    pub action: ShinkaiSubscriptionAction,
    pub subscription_id: Option<String>,
    pub vector_db_path: String,
    pub subscriber_identity: ShinkaiName,
    pub state: Option<String>,
    pub date_created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub last_sync: Option<DateTime<Utc>>,
}

impl PartialOrd for ShinkaiSubscription {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ShinkaiSubscription {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_created.cmp(&other.date_created)
    }
}
