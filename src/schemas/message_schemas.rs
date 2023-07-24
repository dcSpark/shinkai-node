use serde::{Serialize, Deserialize};
use super::inbox_name::InboxName;
use serde_json::Result;

#[derive(Debug)]
pub enum MessageSchemaType {
    JobCreationSchema,
    JobMessageSchema,
    PreMessageSchema,
    PureText,
}

impl MessageSchemaType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "JobCreationSchema" => Some(Self::JobCreationSchema),
            "JobMessageSchema" => Some(Self::JobMessageSchema),
            "PreMessageSchema" => Some(Self::PreMessageSchema),
            "TextContent" => Some(Self::PureText),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::JobCreationSchema => "JobCreationSchema",
            Self::JobMessageSchema => "JobMessageSchema",
            Self::PreMessageSchema => "PreMessageSchema",
            Self::PureText => "PureText",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobScope {
    pub buckets: Vec<InboxName>,
    pub documents: Vec<String>,
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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobCreation {
    pub scope: JobScope,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobMessage {
    pub job_id: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobToolCall {
    pub tool_id: String,
    pub inputs: std::collections::HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobPreMessage {
    pub tool_calls: Vec<JobToolCall>,
    pub content: String,
    pub recipient: String,
}
