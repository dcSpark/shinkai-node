use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use wasm_bindgen::prelude::*;
use crate::shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ShinkaiName {
    pub full_name: String,
    pub node_name: String,
    pub profile_name: Option<String>,
    pub subidentity_type: Option<ShinkaiSubidentityType>,
    pub subidentity_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

#[derive(Debug, PartialEq)]
pub enum ShinkaiNameError {
    MissingBody(String),
    MissingInternalMetadata(String),
    MetadataMissing,
    MessageBodyMissing,
    InvalidGroupFormat(String),
    InvalidNameFormat(String),
    SomeError(String),
    InvalidOperation(String),
}

impl fmt::Display for ShinkaiNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiNameError::MissingBody(message) => {
                write!(f, "Missing body in ShinkaiMessage: {}", message)
            }
            ShinkaiNameError::MissingInternalMetadata(message) => {
                write!(f, "Missing internal metadata in ShinkaiMessage: {}", message)
            }
            ShinkaiNameError::MetadataMissing => write!(f, "Metadata missing"),
            ShinkaiNameError::MessageBodyMissing => write!(f, "Message body missing"),
            ShinkaiNameError::InvalidGroupFormat(message) => {
                write!(f, "Invalid group format: {}", message)
            }
            ShinkaiNameError::InvalidNameFormat(message) => {
                write!(f, "Invalid name format: {}", message)
            }
            ShinkaiNameError::SomeError(message) => write!(f, "Some error: {}", message),
            ShinkaiNameError::InvalidOperation(message) => {
                write!(f, "Invalid operation: {}", message)
            }
        }
    }
}

impl std::error::Error for ShinkaiNameError {}

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
        Self::validate_name(&raw_name)?;

        let parts: Vec<&str> = raw_name.split('/').collect();
        let node_name = parts[0].to_string();
        let profile_name = parts.get(1).map(|s| s.to_string());
        let subidentity_type = parts.get(2).map(|s| {
            if *s == "agent" {
                ShinkaiSubidentityType::Agent
            } else if *s == "device" {
                ShinkaiSubidentityType::Device
            } else {
                panic!("Invalid subidentity type");
            }
        });
        let subidentity_name = parts.get(3).map(|s| s.to_string());

        Ok(Self {
            full_name: raw_name.to_lowercase(),
            node_name: node_name.to_lowercase(),
            profile_name: profile_name.map(|s| s.to_lowercase()),
            subidentity_type,
            subidentity_name,
        })
    }

    pub fn is_fully_valid(shinkai_name: String) -> bool {
        match Self::validate_name(&shinkai_name) {
            Ok(_) => true,
            Err(err) => {
                eprintln!("### Validation error: {}", err);
                false
            }
        }
    }

    pub fn validate_name(raw_name: &str) -> Result<(), &'static str> {
        let parts: Vec<&str> = raw_name.split('/').collect();

        if !(parts.len() >= 1 && parts.len() <= 4) {
            eprintln!(
                "Name should have one to four parts: node, profile, type (device or agent), and name: {}",
                raw_name
            );
            return Err("Name should have one to four parts: node, profile, type (device or agent), and name.");
        }

        if !parts[0].starts_with("@@") || !parts[0].ends_with(".shinkai") {
            eprintln!("### Validation error: {}", raw_name);
            return Err("Node part of the name should start with '@@' and end with '.shinkai'.");
        }

        if !Regex::new(r"^@@[a-zA-Z0-9\_\.]+\.shinkai$").unwrap().is_match(parts[0]) {
            eprintln!("Node part of the name contains invalid characters: {}", raw_name);
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

            if index == 2
                && !(part == &ShinkaiSubidentityType::Agent.to_string()
                    || part == &ShinkaiSubidentityType::Device.to_string())
            {
                return Err("The third part should either be 'agent' or 'device'.");
            }

            if index == 3 && !re.is_match(part) {
                return Err("The fourth part (name after 'agent' or 'device') should be alphanumeric or underscore.");
            }

            if index != 0 && index != 2 && (!re.is_match(part) || part.contains(".shinkai")) {
                return Err("Name parts should be alphanumeric or underscore and not contain '.shinkai'.");
            }
        }

        if parts.len() == 3
            && (parts[2] == &ShinkaiSubidentityType::Agent.to_string()
                || parts[2] == &ShinkaiSubidentityType::Device.to_string())
        {
            return Err("If type is 'agent' or 'device', a fourth part is expected.");
        }

        Ok(())
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

        // Create a new ShinkaiName
        Self::new(full_identity_name)
    }

    pub fn from_shinkai_message_only_using_sender_node_name(message: &ShinkaiMessage) -> Result<Self, &'static str> {
        Self::new(message.external_metadata.sender.clone())
    }

    pub fn from_shinkai_message_only_using_recipient_node_name(message: &ShinkaiMessage) -> Result<Self, &'static str> {
        Self::new(message.external_metadata.recipient.clone())
    }

    pub fn from_shinkai_message_using_sender_subidentity(message: &ShinkaiMessage) -> Result<Self, ShinkaiNameError> {
        // Check if outer encrypted and return error if so
        let body = match &message.body {
            MessageBody::Unencrypted(body) => body,
            _ => return Err(ShinkaiNameError::MessageBodyMissing),
        };

        let node = match Self::new(message.external_metadata.sender.clone()) {
            Ok(name) => name,
            Err(_) => {
                return Err(ShinkaiNameError::InvalidNameFormat(
                    message.external_metadata.sender.clone(),
                ))
            }
        };

        let sender_subidentity = if body.internal_metadata.sender_subidentity.is_empty() {
            String::from("")
        } else {
            format!("/{}", body.internal_metadata.sender_subidentity)
        };

        match Self::new(format!("{}{}", node, sender_subidentity)) {
            Ok(name) => Ok(name),
            Err(_) => Err(ShinkaiNameError::InvalidNameFormat(format!(
                "{}{}",
                node, sender_subidentity
            ))),
        }
    }

    pub fn from_shinkai_message_using_recipient_subidentity(message: &ShinkaiMessage) -> Result<Self, ShinkaiNameError> {
        // Check if the message is encrypted
        let body = match &message.body {
            MessageBody::Unencrypted(body) => body,
            _ => return Err(ShinkaiNameError::InvalidOperation(
                "Cannot process encrypted ShinkaiMessage".to_string(),
            )),
        };
    
        let node = match Self::new(message.external_metadata.recipient.clone()) {
            Ok(name) => name,
            Err(_) => return Err(ShinkaiNameError::InvalidNameFormat(
                message.external_metadata.recipient.clone(),
            )),
        };
    
        let recipient_subidentity = if body.internal_metadata.recipient_subidentity.is_empty() {
            String::from("")
        } else {
            format!("/{}", body.internal_metadata.recipient_subidentity)
        };
    
        match Self::new(format!("{}{}", node, recipient_subidentity)) {
            Ok(name) => Ok(name),
            Err(_) => Err(ShinkaiNameError::InvalidNameFormat(format!(
                "{}{}",
                node, recipient_subidentity
            ))),
        }
    }

    // This method checks if a name is a valid node identity name and doesn't contain subidentities
    fn is_valid_node_identity_name_and_no_subidentities(name: &String) -> bool {
        // A node name is valid if it starts with '@@', ends with '.shinkai', and doesn't contain '/'
        name.starts_with("@@") && name.ends_with(".shinkai") && !name.contains("/")
    }

    pub fn contains(&self, other: &ShinkaiName) -> bool {
        let self_parts: Vec<&str> = self.full_name.split('/').collect();
        let other_parts: Vec<&str> = other.full_name.split('/').collect();

        if self_parts.len() > other_parts.len() {
            return false;
        }

        self_parts
            .iter()
            .zip(other_parts.iter())
            .all(|(self_part, other_part)| self_part == other_part)
    }

    pub fn has_profile(&self) -> bool {
        self.profile_name.is_some()
    }

    pub fn has_device(&self) -> bool {
        match self.subidentity_type {
            Some(ShinkaiSubidentityType::Device) => true,
            _ => false,
        }
    }

    pub fn has_agent(&self) -> bool {
        match self.subidentity_type {
            Some(ShinkaiSubidentityType::Agent) => true,
            _ => false,
        }
    }

    pub fn has_no_subidentities(&self) -> bool {
        self.profile_name.is_none() && self.subidentity_type.is_none()
    }

    pub fn get_profile_name(&self) -> Option<String> {
        self.profile_name.clone()
    }

    pub fn get_node_name(&self) -> String {
        self.node_name.clone()
    }

    pub fn get_device_name(&self) -> Option<String> {
        if self.has_device() {
            self.subidentity_name.clone()
        } else {
            None
        }
    }

    pub fn get_agent_name(&self) -> Option<String> {
        if self.has_agent() {
            self.subidentity_name.clone()
        } else {
            None
        }
    }

    pub fn extract_profile(&self) -> Result<Self, &'static str> {
        if self.has_no_subidentities() {
            return Err("This ShinkaiName does not include a profile.");
        }

        Ok(Self {
            full_name: format!("{}/{}", self.node_name, self.profile_name.as_ref().unwrap()),
            node_name: self.node_name.clone(),
            profile_name: self.profile_name.clone(),
            subidentity_type: None,
            subidentity_name: None,
        })
    }

    pub fn extract_node(&self) -> Self {
        Self {
            full_name: self.node_name.clone(),
            node_name: self.node_name.clone(),
            profile_name: None,
            subidentity_type: None,
            subidentity_name: None,
        }
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

    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self).map_err(|e| JsValue::from_str(&e.to_string()))?)
    }

    pub fn from_jsvalue(j: &JsValue) -> Result<Self, JsValue> {
        Ok(serde_wasm_bindgen::from_value(j.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?)
    }

    pub fn to_json_str(&self) -> Result<String, JsValue> {
        let json_str = serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(json_str)
    }

    pub fn from_json_str(j: &str) -> Result<Self, JsValue> {
        let shinkai_name = serde_json::from_str(j).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(shinkai_name)
    }
}

impl fmt::Display for ShinkaiName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.full_name)
    }
}

impl AsRef<str> for ShinkaiName {
    fn as_ref(&self) -> &str {
        &self.full_name
    }
}
