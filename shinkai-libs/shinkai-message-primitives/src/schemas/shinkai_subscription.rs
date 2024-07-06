use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{shinkai_name::ShinkaiName, shinkai_subscription_req::SubscriptionPayment};
use shinkai_vector_resources::vector_resource::VRPath;

// TODO: This should have the fields stored separate, and just have get unique id build the id string. Moves validation to from_unique_id as it should be.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId {
    pub unique_id: String,
    pub include_folders: Option<Vec<VRPath>>,
    pub exclude_folders: Option<Vec<VRPath>>,
}

impl SubscriptionId {
    pub fn new(
        streamer_node: ShinkaiName,
        streamer_profile: String,
        shared_folder: String,
        subscriber_node: ShinkaiName,
        subscriber_profile: String,
    ) -> Self {
        // Check if origin_node and subscriber_node are the same
        let streamer_node_str = streamer_node.get_node_name_string();
        let subscriber_node_str = subscriber_node.get_node_name_string();
        if streamer_node_str == subscriber_node_str {
            panic!("streamer_node and subscriber_node cannot be the same");
        }

        let streamer_node_str = streamer_node.get_node_name_string();
        let subscriber_node_str = subscriber_node.get_node_name_string();
        let unique_id = format!(
            "{}:::{}:::{}:::{}:::{}",
            streamer_node_str, streamer_profile, shared_folder, subscriber_node_str, subscriber_profile
        );
        SubscriptionId {
            unique_id,
            include_folders: None,
            exclude_folders: None,
        }
    }

    pub fn from_unique_id(unique_id: String) -> Self {
        SubscriptionId {
            unique_id,
            include_folders: None,
            exclude_folders: None,
        }
    }

    pub fn get_unique_id(&self) -> &str {
        &self.unique_id
    }

    pub fn fixed_deterministic_identifier(&self) -> String {
        let full_hash = blake3::hash(self.get_unique_id().as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    // Update existing extract methods to check for 5 parts instead of 3
    /// Extracts the shared folder from the unique_id of the SubscriptionId.
    pub fn extract_shared_folder(&self) -> Result<String, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            Ok(parts[2].to_string())
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the node name with shared folder from the unique_id of the SubscriptionId.
    pub fn extract_streamer_node(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            Ok(ShinkaiName::new(parts[0].to_string())?)
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the shared folder origin node profile from the unique_id of the SubscriptionId.
    pub fn extract_streamer_profile(&self) -> Result<String, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            Ok(parts[1].to_string())
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the streamer node and profile from the unique_id of the SubscriptionId
    /// and tries to create a new ShinkaiName using them.
    pub fn extract_streamer_node_with_profile(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            let streamer_node_str = parts[0];
            let streamer_profile = parts[1];
            ShinkaiName::new(format!("{}/{}", streamer_node_str, streamer_profile))
                .map_err(|_| "Failed to create ShinkaiName from streamer node and profile")
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the node name of the subscriber from the unique_id of the SubscriptionId.
    pub fn extract_subscriber_node(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            Ok(ShinkaiName::new(parts[3].to_string())?)
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Extracts the subscriber profile from the unique_id of the SubscriptionId.
    pub fn extract_subscriber_profile(&self) -> Result<String, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            Ok(parts[4].to_string())
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    /// Correctly extracts the subscriber node and profile from the unique_id of the SubscriptionId
    /// and tries to create a new ShinkaiName using them.
    pub fn extract_subscriber_node_with_profile(&self) -> Result<ShinkaiName, &'static str> {
        let parts: Vec<&str> = self.unique_id.split(":::").collect();
        if parts.len() == 5 {
            let subscriber_node_str = parts[3];
            let subscriber_profile = parts[4];
            ShinkaiName::new(format!("{}/{}", subscriber_node_str, subscriber_profile))
                .map_err(|_| "Failed to create ShinkaiName from subscriber node and profile")
        } else {
            Err("Invalid SubscriptionId format")
        }
    }

    // Method to update include_folders
    pub fn update_include_folders(&mut self, folders: Vec<VRPath>) {
        self.include_folders = Some(folders);
    }

    // Method to update exclude_folders
    pub fn update_exclude_folders(&mut self, folders: Vec<VRPath>) {
        self.exclude_folders = Some(folders);
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
    pub streaming_node: ShinkaiName,
    pub streaming_profile: String,
    pub subscription_description: Option<String>,
    pub subscriber_destination_path: Option<String>,
    pub subscriber_node: ShinkaiName,
    pub subscriber_profile: String,
    pub payment: Option<SubscriptionPayment>,
    pub state: ShinkaiSubscriptionStatus,
    pub date_created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub last_sync: Option<DateTime<Utc>>,
    pub http_preferred: Option<bool>,
}

impl ShinkaiSubscription {
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn new(
        shared_folder: String,
        streaming_node: ShinkaiName,
        streaming_profile: String,
        subscriber_node: ShinkaiName,
        subscriber_profile: String,
        state: ShinkaiSubscriptionStatus,
        payment: Option<SubscriptionPayment>,
        base_folder: Option<String>,
        subscription_description: Option<String>,
    ) -> Self {
        ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                streaming_node.clone(),
                streaming_profile.clone(),
                shared_folder.clone(),
                subscriber_node.clone(),
                subscriber_profile.clone(),
            ),
            shared_folder,
            streaming_node,
            streaming_profile,
            subscription_description,
            subscriber_destination_path: base_folder,
            subscriber_node,
            subscriber_profile,
            payment,
            state,
            date_created: Utc::now(),
            last_modified: Utc::now(),
            last_sync: None,
            http_preferred: None,
        }
    }

    // Method to update the http_preferred field
    #[allow(dead_code)]
    pub fn update_http_preferred(&mut self, preferred: Option<bool>) {
        self.http_preferred = preferred;
        self.last_modified = Utc::now();
    }

    #[allow(dead_code)]
    pub fn with_state(mut self, new_state: ShinkaiSubscriptionStatus) -> Self {
        self.state = new_state;
        self.last_modified = Utc::now();
        self
    }

    /// Returns the subscriber with profile as ShinkaiName.
    pub fn get_subscriber_with_profile(&self) -> Result<ShinkaiName, &'static str> {
        ShinkaiName::from_node_and_profile_names(
            self.subscriber_node.full_name.clone(),
            self.subscriber_profile.clone(),
        )
        .map_err(|_| "Failed to create ShinkaiName from subscriber node and profile")
    }

    /// Returns the streamer with profile as ShinkaiName.
    pub fn get_streamer_with_profile(&self) -> Result<ShinkaiName, &'static str> {
        ShinkaiName::from_node_and_profile_names(self.streaming_node.full_name.clone(), self.streaming_profile.clone())
            .map_err(|_| "Failed to create ShinkaiName from streamer node and profile")
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
