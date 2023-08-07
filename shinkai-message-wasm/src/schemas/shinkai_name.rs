use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShinkaiName(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShinkaiSubidentityType {
    Agent,
    Device,
}

impl fmt::Display for ShinkaiSubidentityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShinkaiSubidentityType::Agent => write!(f, "agent"),
            ShinkaiSubidentityType::Device => write!(f, "device"),
        }
    }
}

#[derive(Debug)]
pub enum ShinkaiNameError {
    MetadataMissing,
    MessageBodyMissing,
    InvalidNameFormat(String),
}

// Valid Examples
// @@alice.shinkai
// @@alice.shinkai/profileName
// @@alice.shinkai/profileName/agent/myChatGPTAgent
// @@alice.shinkai/profileName/device/myPhone

// Not valid examples
// @@alice.shinkai/profileName/myPhone
// @@al!ce.shinkai
// @@alice.shinkai//
// @@node1.shinkai/profile_1.shinkai

impl ShinkaiName {
    pub fn new(raw_name: String) -> Result<Self, &'static str> {
        let raw_name = Self::correct_node_name(raw_name);
        let parts: Vec<&str> = raw_name.split('/').collect();
    
        if !(parts.len() >= 1 && parts.len() <= 4) {
            return Err("Name should have one to four parts: node, profile, type (device or agent), and name.");
        }
    
        if !parts[0].starts_with("@@") || !parts[0].ends_with(".shinkai") {
            return Err("Node part of the name should start with '@@' and end with '.shinkai'.");
        }

        if !Regex::new(r"^@@[a-zA-Z0-9\-\.]+\.shinkai$").unwrap().is_match(parts[0]) {
            return Err("Node part of the name contains invalid characters.");
        }
    
        let re = Regex::new(r"^[a-zA-Z0-9_]*$").unwrap();
    
        for (index, part) in parts.iter().enumerate() {
            if index == 0 {
                if part.contains("/") {
                    return Err("Root node name cannot contain '/'.");
                }
                continue;
            }
            
            if index == 2 && !(part == &ShinkaiSubidentityType::Agent.to_string() || part == &ShinkaiSubidentityType::Device.to_string()) {
                return Err("The third part should either be 'agent' or 'device'.");
            }
    
            if index == 3 && !re.is_match(part) {
                return Err("The fourth part (name after 'agent' or 'device') should be alphanumeric or underscore.");
            }
            
            if index != 0 && index != 2 && (!re.is_match(part) || part.contains(".shinkai")) {
                return Err("Name parts should be alphanumeric or underscore and not contain '.shinkai'.");
            }
        }
    
        if parts.len() == 3 && (parts[2] == &ShinkaiSubidentityType::Agent.to_string() || parts[2] == &ShinkaiSubidentityType::Device.to_string()) {
            return Err("If type is 'agent' or 'device', a fourth part is expected.");
        }
    
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
        let node_name = Self::correct_node_name(node_name);

        // Construct the full_identity_name
        let full_identity_name = format!("{}/{}", node_name.to_lowercase(), profile_name.to_lowercase());

        // Create a new ShinkaiName
        Self::new(full_identity_name)
    }

    pub fn from_node_and_profile_and_type_and_name(
        node_name: String,
        profile_name: String,
        shinkai_type: ShinkaiSubidentityType,
        name: String,
    ) -> Result<Self, &'static str> {
        // Validate and format the node_name
        let node_name = Self::correct_node_name(node_name);

        let shinkai_type_str = shinkai_type.to_string();

        // Construct the full_identity_name
        let full_identity_name = format!(
            "{}/{}/{}/{}",
            node_name.to_lowercase(),
            profile_name.to_lowercase(),
            shinkai_type_str,
            name.to_lowercase()
        );

        println!("device full_identity_name: {}", full_identity_name);

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
        let parts: Vec<&str> = self.0.split('/').collect();
        parts.contains(&"device")
    }

    pub fn has_no_subidentities(&self) -> bool {
        // If it contains no '/' then it's only a node
        !self.0.contains('/')
    }

    pub fn get_profile_name(&self) -> Option<String> {
        if !self.has_profile() {
            return None;
        }
    
        let parts: Vec<&str> = self.0.split('/').collect();
        // Assuming that parts[0] is always the node name and parts[1] is the profile name
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

impl ShinkaiName {
    fn correct_node_name(raw_name: String) -> String {
        let mut parts: Vec<&str> = raw_name.splitn(2, '/').collect();
        
        let mut node_name = parts[0].to_string();
        
        // Prepend with "@@" if the node doesn't already start with "@@"
        if !node_name.starts_with("@@") {
            node_name = format!("@@{}", node_name);
        }

        // Append with ".shinkai" if the node doesn't already end with ".shinkai"
        if !node_name.ends_with(".shinkai") {
            node_name = format!("{}.shinkai", node_name);
        }

        // Reconstruct the name
        let corrected_name = if parts.len() > 1 {
            format!("{}/{}", node_name, parts[1])
        } else {
            node_name
        };

        corrected_name
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
