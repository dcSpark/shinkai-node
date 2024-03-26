use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{shinkai_name::ShinkaiName, shinkai_subscription_req::SubscriptionPayment};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SubscriptionId {
    unique_id: String,
}

impl SubscriptionId {
    pub fn new(
        shared_folder_origin_node_name: ShinkaiName,
        shared_folder_origin_node_profile: String,
        shared_folder: String,
        node_name_subscriber: ShinkaiName,
    ) -> Self {
        // Check if shared_folder_origin_node_name and node_name_subscriber are the same
        let shared_folder_origin_node_name_str = shared_folder_origin_node_name.get_node_name();
        let node_name_subscriber_str = node_name_subscriber.get_node_name();
        if shared_folder_origin_node_name_str == node_name_subscriber_str {
            panic!("shared_folder_origin_node_name and node_name_subscriber cannot be the same");
        }

        let shared_folder_origin_node_name_str = shared_folder_origin_node_name.get_node_name();
        let node_name_subscriber_str = node_name_subscriber.get_node_name();
        let unique_id = format!(
            "{}:::{}:::{}",
            shared_folder_origin_node_name_str, shared_folder, node_name_subscriber_str
        );
        SubscriptionId { unique_id }
    }

    pub fn from_unique_id(unique_id: String) -> Self {
        SubscriptionId { unique_id }
    }

    pub fn get_unique_id(&self) -> &str {
        &self.unique_id
    }

    pub fn fixed_deterministic_identifier(&self) -> String {
        let full_hash = blake3::hash(self.get_unique_id().as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Extracts the shared folder from the unique_id of the SubscriptionId.
    pub fn extract_shared_folder(&self) -> Result<String, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 3 {
            Ok(parts[1].to_string())
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the node name with shared folder from the unique_id of the SubscriptionId.
    pub fn extract_shared_folder_origin_node_name(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 3 {
            Ok(ShinkaiName::new(parts[0].to_string())?)
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the node name of the subscriber from the unique_id of the SubscriptionId.
    pub fn extract_node_name_subscriber(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 3 {
            Ok(ShinkaiName::new(parts[2].to_string())?)
        } else {
            Err("Invalid SubscriptionId format")
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ShinkaiSubscriptionStatus {
    SubscriptionRequested,
    SubscriptionConfirmed,
    UnsubscribeRequested,
    UnsubscribeConfirmed,
    UpdateSubscriptionRequested,
    UpdateSubscriptionConfirmed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ShinkaiSubscription {
    pub subscription_id: SubscriptionId,
    pub shared_folder: String,
    pub shared_folder_origin_node_name: ShinkaiName,
    pub shared_folder_origin_node_profile: String,
    pub subscription_description: Option<String>,
    pub subscriber_destination_path: Option<String>,
    pub subscriber_identity: ShinkaiName,
    pub payment: Option<SubscriptionPayment>,
    pub state: ShinkaiSubscriptionStatus,
    pub date_created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub last_sync: Option<DateTime<Utc>>,
}

impl ShinkaiSubscription {
    pub fn new(
        shared_folder: String,
        shared_folder_origin_node_name: ShinkaiName,
        shared_folder_origin_node_profile: String,
        subscriber_identity: ShinkaiName,
        state: ShinkaiSubscriptionStatus,
        payment: Option<SubscriptionPayment>,
    ) -> Self {
        ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                shared_folder_origin_node_name.clone(),
                shared_folder_origin_node_profile.clone(),
                shared_folder.clone(),
                subscriber_identity.clone(),
            ),
            shared_folder,
            shared_folder_origin_node_name,
            subscription_description: None, // TODO: update api and models to support this
            subscriber_destination_path: None, // TODO: update api to support this
            subscriber_identity,
            payment,
            state,
            date_created: Utc::now(),
            last_modified: Utc::now(),
            last_sync: None,
        }
    }

    pub fn with_state(mut self, new_state: ShinkaiSubscriptionStatus) -> Self {
        self.state = new_state;
        self.last_modified = Utc::now();
        self
    }
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
