use serde::{Deserialize, Serialize};
use serde_json::Result;
use regex::Regex;

use crate::schemas::inbox_name::InboxName;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MessageSchemaType {
    JobCreationSchema,
    JobMessageSchema,
    PreMessageSchema,
    TextContent,
    Empty
}

impl MessageSchemaType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "JobCreationSchema" => Some(Self::JobCreationSchema),
            "JobMessageSchema" => Some(Self::JobMessageSchema),
            "PreMessageSchema" => Some(Self::PreMessageSchema),
            "TextContent" => Some(Self::TextContent),
            "" => Some(Self::Empty),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::JobCreationSchema => "JobCreationSchema",
            Self::JobMessageSchema => "JobMessageSchema",
            Self::PreMessageSchema => "PreMessageSchema",
            Self::TextContent => "TextContent",
            Self::Empty => "",
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JobScope {
    pub buckets: Vec<InboxName>,
    pub documents: Vec<String>, // TODO: link to embedding of documents uploaded
}

impl JobScope {
    pub fn new(buckets: Option<Vec<InboxName>>, documents: Option<Vec<String>>) -> Self {
        Self {
            buckets: buckets.unwrap_or_else(Vec::<InboxName>::new),
            documents: documents.unwrap_or_else(Vec::new),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let j = serde_json::to_string(self)?;
        Ok(j.into_bytes())
    }

    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    pub fn from_json_str(s: &str) -> Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobCreation {
    pub scope: JobScope,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobMessage {
    // TODO: scope div modifications?
    pub job_id: String,
    pub content: String,
}

impl JobMessage {
    pub fn from_json_str(s: &str) -> Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JobToolCall {
    pub tool_id: String,
    pub inputs: std::collections::HashMap<String, String>,
}

impl JobToolCall {
    pub fn from_json_str(s: &str) -> Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum JobRecipient {
    SelfNode,
    User,
    ExternalIdentity(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JobPreMessage {
    pub tool_calls: Vec<JobToolCall>,
    pub content: String,
    pub recipient: JobRecipient,
}

impl JobPreMessage {
    pub fn from_json_str(s: &str) -> Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}

impl JobRecipient {
    // TODO: this should be moved to a more general place
    pub fn is_valid_node_identity_name_with_subidentities(s: &str) -> bool {
        let re = Regex::new(r"^@@[^/]+\.shinkai(/[^/]*)*$").unwrap();
        re.is_match(s)
    }

    pub fn validate_external(&self) -> std::result::Result<(), &'static str> {
        match self {
            Self::ExternalIdentity(identity) => {
                if JobRecipient::is_valid_node_identity_name_with_subidentities(identity) {
                    Ok(())
                } else {
                    Err("Invalid identity")
                }
            }
            _ => Ok(()), // For other variants we do not perform validation, so return Ok
        }
    }

    pub fn from_json_str(s: &str) -> Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}
