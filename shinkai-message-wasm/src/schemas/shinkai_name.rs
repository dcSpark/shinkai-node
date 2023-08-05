use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShinkaiName(String);

// Name Examples
// @@alice.shinkai
// @@alice.shinkai/profileName
// @@alice.shinkai/profileName/myChatGPTAgent
// @@alice.shinkai/profileName/myPhone

#[derive(Debug)]
pub enum ShinkaiNameError {
    MetadataMissing,
    MessageBodyMissing,
    InvalidNameFormat(String),
}

impl ShinkaiName {
    pub fn new(mut raw_name: String) -> Result<Self, &'static str> {
        // Prepend with "@@" if it doesn't already start with "@@"
        if !raw_name.starts_with("@@") {
            raw_name = format!("@@{}", raw_name);
        }

        // Append with ".shinkai" if it doesn't already end with ".shinkai"
        if !raw_name.ends_with(".shinkai") {
            raw_name = format!("{}.shinkai", raw_name);
        }

        // Check if the base name is alphanumeric or contains underscores
        let base_name_parts: Vec<&str> = raw_name.split('.').collect();
        let base_name = base_name_parts.get(0).unwrap().trim_start_matches("@@");
        let re = Regex::new(r"^[a-zA-Z0-9_]*$").unwrap();
        if !re.is_match(base_name) {
            return Err("Base name should be alphanumeric and can include underscores.");
        }

        // Split by '/' and check if it has one to three parts: node, profile, and optional device
        let parts: Vec<&str> = raw_name.split('/').collect();
        if !(parts.len() >= 1 && parts.len() <= 3) {
            return Err("Name should have one to three parts: node, profile, and optional device.");
        }

        // Check if the node part starts with '@@' and ends with '.shinkai'
        if !parts[0].starts_with("@@") || !parts[0].ends_with(".shinkai") {
            return Err("Node part of the name should start with '@@' and end with '.shinkai'.");
        }

        // If all checks passed, create a new ShinkaiName instance
        Ok(Self(raw_name.to_lowercase()))
    }

    pub fn from_node_name(node_name: String) -> Result<Self, ShinkaiNameError> {
        // Ensure the node_name has no forward slashes
        if node_name.contains("/") {
            return Err(ShinkaiNameError::InvalidNameFormat(node_name.clone()));
        }
        let node_name_clone = node_name.clone();
        // Use the existing new() method to handle the rest of the formatting and checks
        match Self::new(node_name_clone) {
            Ok(name) => Ok(name),
            Err(_) => Err(ShinkaiNameError::InvalidNameFormat(node_name.clone())),
        }
    }

    pub fn from_node_and_profile(node_name: String, profile_name: String) -> Result<Self, &'static str> {
        // Validate and format the node_name
        let node_name = if Self::is_valid_node_identity_name_and_no_subidentities(&node_name) {
            node_name
        } else {
            format!("@@{}.shinkai", node_name)
        };

        // Construct the full_identity_name
        let full_identity_name = format!("{}/{}", node_name.to_lowercase(), profile_name.to_lowercase());

        // Create a new ShinkaiName
        Self::new(full_identity_name)
    }

    pub fn from_node_and_profile_and_device(
        node_name: String,
        profile_name: String,
        device_name: String,
    ) -> Result<Self, &'static str> {
        // Validate and format the node_name
        let node_name = if Self::is_valid_node_identity_name_and_no_subidentities(&node_name) {
            node_name
        } else {
            format!("@@{}.shinkai", node_name)
        };

        // Construct the full_identity_name
        let full_identity_name = format!(
            "{}/{}/{}",
            node_name.to_lowercase(),
            profile_name.to_lowercase(),
            device_name.to_lowercase()
        );

        // Create a new ShinkaiName
        Self::new(full_identity_name)
    }

    pub fn from_shinkai_message_using_sender(message: &ShinkaiMessage) -> Result<Self, &'static str> {
        match &message.external_metadata {
            Some(metadata) => Self::new(metadata.sender.clone()),
            None => Err("External metadata is missing."),
        }
    }

    pub fn from_shinkai_message_using_recipient(message: &ShinkaiMessage) -> Result<Self, &'static str> {
        match &message.external_metadata {
            Some(metadata) => Self::new(metadata.recipient.clone()),
            None => Err("External metadata is missing."),
        }
    }

    pub fn from_shinkai_message_using_sender_subidentity(message: &ShinkaiMessage) -> Result<Self, ShinkaiNameError> {
        match (&message.body, &message.external_metadata) {
            (Some(body), Some(external_metadata)) => match &body.internal_metadata {
                Some(metadata) => {
                    let node = match Self::new(external_metadata.sender.clone()) {
                        Ok(name) => name.extract_node(),
                        Err(_) => return Err(ShinkaiNameError::InvalidNameFormat(external_metadata.sender.clone())),
                    };
                    match Self::new(format!("{}/{}", node, metadata.sender_subidentity)) {
                        Ok(name) => Ok(name),
                        Err(_) => Err(ShinkaiNameError::InvalidNameFormat(format!(
                            "{}/{}",
                            node, metadata.sender_subidentity
                        ))),
                    }
                }
                None => Err(ShinkaiNameError::MetadataMissing),
            },
            _ => Err(ShinkaiNameError::MessageBodyMissing),
        }
    }

    pub fn from_shinkai_message_using_recipient_subidentity(
        message: &ShinkaiMessage,
    ) -> Result<Self, ShinkaiNameError> {
        match (&message.body, &message.external_metadata) {
            (Some(body), Some(external_metadata)) => match &body.internal_metadata {
                Some(metadata) => {
                    let node = match Self::new(external_metadata.recipient.clone()) {
                        Ok(name) => name.extract_node(),
                        Err(_) => return Err(ShinkaiNameError::InvalidNameFormat(external_metadata.recipient.clone())),
                    };
                    match Self::new(format!("{}/{}", node, metadata.recipient_subidentity)) {
                        Ok(name) => Ok(name),
                        Err(_) => Err(ShinkaiNameError::InvalidNameFormat(format!(
                            "{}/{}",
                            node, metadata.recipient_subidentity
                        ))),
                    }
                }
                None => Err(ShinkaiNameError::MetadataMissing),
            },
            _ => Err(ShinkaiNameError::MessageBodyMissing),
        }
    }

    // This method checks if a name is a valid node identity name and doesn't contain subidentities
    fn is_valid_node_identity_name_and_no_subidentities(name: &String) -> bool {
        // A node name is valid if it starts with '@@', ends with '.shinkai', and doesn't contain '/'
        name.starts_with("@@") && name.ends_with(".shinkai") && !name.contains("/")
    }

    pub fn has_profile(&self) -> bool {
        // Check if it contains two '/' (indicating it has a profile)
        self.0.matches('/').count() >= 1
    }

    pub fn has_device(&self) -> bool {
        // Check if it contains two '/' (indicating it has a device)
        self.0.matches('/').count() == 2
    }

    pub fn has_no_subidentities(&self) -> bool {
        // If it contains no '/' then it's only a node
        !self.0.contains('/')
    }

    pub fn get_profile_name(&self) -> Option<String> {
        if !self.has_profile() {
            return None;
        }

        let parts: Vec<&str> = self.0.splitn(2, '/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai', and parts[1] contains the profile name

        Some(parts[1].to_string())
    }

    pub fn get_node_name(&self) -> String {
        let parts: Vec<&str> = self.0.split('/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai'
        parts[0].to_string()
    }

    pub fn get_device_name(&self) -> Option<String> {
        if !self.has_device() {
            return None;
        }

        let parts: Vec<&str> = self.0.rsplitn(2, '/').collect();
        // parts[0] now contains the device name
        Some(parts[0].to_string())
    }

    pub fn extract_profile(&self) -> Result<Self, &'static str> {
        if self.has_no_subidentities() {
            return Err("This ShinkaiName does not include a profile.");
        }

        let parts: Vec<&str> = self.0.splitn(2, '/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai', and parts[1] contains the profile name

        // Form a new ShinkaiName with only the node and profile, but no device
        Self::new(format!("{}/{}", parts[0], parts[1]))
    }

    pub fn extract_node(&self) -> Self {
        let parts: Vec<&str> = self.0.split('/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai'
        let node_name = parts[0].to_string();
    
        // create a new ShinkaiName instance from the extracted node_name
        Self::new(node_name).unwrap()
    }
}

impl fmt::Display for ShinkaiName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<[u8]> for ShinkaiName {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
