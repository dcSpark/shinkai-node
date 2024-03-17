use crate::schemas::shinkai_subscription::{ShinkaiSubscription, ShinkaiSubscriptionRequest};
use crate::schemas::shinkai_subscription_req::ShinkaiFolderSubscription;
use crate::schemas::{agents::serialized_agent::SerializedAgent, inbox_name::InboxName, shinkai_name::ShinkaiName};
use crate::shinkai_utils::job_scope::JobScope;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Result;
use std::fmt;

use super::shinkai_message::ShinkaiMessage;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MessageSchemaType {
    JobCreationSchema,
    JobMessageSchema,
    PreMessageSchema,
    CreateRegistrationCode,
    UseRegistrationCode,
    APIGetMessagesFromInboxRequest,
    APIReadUpToTimeRequest,
    APIAddAgentRequest,
    APIFinishJob,
    TextContent,
    ChangeNodesName,
    WSMessage,
    FormattedMultiContent, // TODO
    SymmetricKeyExchange,
    EncryptedFileContent,
    Empty,
    VecFsRetrievePathSimplifiedJson,
    VecFsRetrieveVectorResource,
    VecFsRetrieveVectorSearchSimplifiedJson,
    VecFsCreateFolder,
    VecFsDeleteFolder,
    VecFsMoveFolder,
    VecFsCopyFolder,
    VecFsCreateItem,
    VecFsMoveItem,
    VecFsCopyItem,
    AvailableSharedItems,
    CreateShareableFolder,
    UpdateShareableFolder,
    UnshareFolder,
    ConvertFilesAndSaveToFolder,
}

impl MessageSchemaType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "JobCreationSchema" => Some(Self::JobCreationSchema),
            "JobMessageSchema" => Some(Self::JobMessageSchema),
            "PreMessageSchema" => Some(Self::PreMessageSchema),
            "CreateRegistrationCode" => Some(Self::CreateRegistrationCode),
            "UseRegistrationCode" => Some(Self::UseRegistrationCode),
            "APIGetMessagesFromInboxRequest" => Some(Self::APIGetMessagesFromInboxRequest),
            "APIReadUpToTimeRequest" => Some(Self::APIReadUpToTimeRequest),
            "APIAddAgentRequest" => Some(Self::APIAddAgentRequest),
            "TextContent" => Some(Self::TextContent),
            "ChangeNodesName" => Some(Self::ChangeNodesName),
            "WSMessage" => Some(Self::WSMessage),
            "FormattedMultiContent" => Some(Self::FormattedMultiContent),
            "SymmetricKeyExchange" => Some(Self::SymmetricKeyExchange),
            "EncryptedFileContent" => Some(Self::EncryptedFileContent),
            "APIFinishJob" => Some(Self::APIFinishJob),
            "" => Some(Self::Empty),
            "VecFsRetrievePathSimplifiedJson" => Some(Self::VecFsRetrievePathSimplifiedJson),
            "VecFsRetrieveVectorResource" => Some(Self::VecFsRetrieveVectorResource),
            "VecFsRetrieveVectorSearchSimplifiedJson" => Some(Self::VecFsRetrieveVectorSearchSimplifiedJson),
            "VecFsCreateFolder" => Some(Self::VecFsCreateFolder),
            "VecFsDeleteFolder" => Some(Self::VecFsDeleteFolder),
            "VecFsMoveFolder" => Some(Self::VecFsMoveFolder),
            "VecFsCopyFolder" => Some(Self::VecFsCopyFolder),
            "VecFsCreateItem" => Some(Self::VecFsCreateItem),
            "VecFsMoveItem" => Some(Self::VecFsMoveItem),
            "VecFsCopyItem" => Some(Self::VecFsCopyItem),
            "AvailableSharedItems" => Some(Self::AvailableSharedItems),
            "CreateShareableFolder" => Some(Self::CreateShareableFolder),
            "UpdateShareableFolder" => Some(Self::UpdateShareableFolder),
            "UnshareFolder" => Some(Self::UnshareFolder),
            "ConvertFilesAndSaveToFolder" => Some(Self::ConvertFilesAndSaveToFolder),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::JobCreationSchema => "JobCreationSchema",
            Self::JobMessageSchema => "JobMessageSchema",
            Self::PreMessageSchema => "PreMessageSchema",
            Self::CreateRegistrationCode => "CreateRegistrationCode",
            Self::UseRegistrationCode => "UseRegistrationCode",
            Self::APIGetMessagesFromInboxRequest => "APIGetMessagesFromInboxRequest",
            Self::APIReadUpToTimeRequest => "APIReadUpToTimeRequest",
            Self::APIAddAgentRequest => "APIAddAgentRequest",
            Self::TextContent => "TextContent",
            Self::ChangeNodesName => "ChangeNodesName",
            Self::WSMessage => "WSMessage",
            Self::FormattedMultiContent => "FormattedMultiContent",
            Self::SymmetricKeyExchange => "SymmetricKeyExchange",
            Self::EncryptedFileContent => "FileContent",
            Self::APIFinishJob => "APIFinishJob",
            Self::VecFsRetrievePathSimplifiedJson => "VecFsRetrievePathSimplifiedJson",
            Self::VecFsRetrieveVectorResource => "VecFsRetrieveVectorResource",
            Self::VecFsRetrieveVectorSearchSimplifiedJson => "VecFsRetrieveVectorSearchSimplifiedJson",
            Self::VecFsCreateFolder => "VecFsCreateFolder",
            Self::VecFsDeleteFolder => "VecFsDeleteFolder",
            Self::VecFsMoveFolder => "VecFsMoveFolder",
            Self::VecFsCopyFolder => "VecFsCopyFolder",
            Self::VecFsCreateItem => "VecFsCreateItem",
            Self::VecFsMoveItem => "VecFsMoveItem",
            Self::VecFsCopyItem => "VecFsCopyItem",
            Self::AvailableSharedItems => "AvailableSharedItems",
            Self::CreateShareableFolder => "CreateShareableFolder",
            Self::UpdateShareableFolder => "UpdateShareableFolder",
            Self::UnshareFolder => "UnshareFolder",
            Self::ConvertFilesAndSaveToFolder => "ConvertFilesAndSaveToFolder",
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
pub struct SymmetricKeyExchange {
    pub shared_secret_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobCreationInfo {
    pub scope: JobScope,
    pub is_hidden: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JobMessage {
    // TODO: scope div modifications?
    pub job_id: String,
    pub content: String,
    pub files_inbox: String,
    pub parent: Option<String>,
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
    pub fn validate_external(&self) -> std::result::Result<(), &'static str> {
        match self {
            Self::ExternalIdentity(identity) => {
                if ShinkaiName::new(identity.to_string()).is_ok() {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIGetMessagesFromInboxRequest {
    pub inbox: String,
    pub count: usize,
    pub offset: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIReadUpToTimeRequest {
    pub inbox_name: InboxName,
    pub up_to_time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIAddAgentRequest {
    pub agent: SerializedAgent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsRetrievePathSimplifiedJson {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIConvertFilesAndSaveToFolder {
    pub path: String,
    pub file_inbox: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFSRetrieveVectorResource {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsRetrieveVectorSearchSimplifiedJson {
    pub search: String,
    pub path: Option<String>,
    pub max_results: Option<usize>,
    pub max_files_to_scan: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsCreateFolder {
    pub path: String,
    pub folder_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsDeleteFolder {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsDeleteItem {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsMoveFolder {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsCopyFolder {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsCreateItem {
    pub path: String,
    pub item_name: String,
    pub item_content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsMoveItem {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsCopyItem {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIAvailableSharedItems {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APICreateShareableFolder {
    pub path: String,
    pub subscription_req: ShinkaiFolderSubscription,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIUpdateShareableFolder {
    pub path: String,
    pub subscription: ShinkaiFolderSubscription,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIUnshareFolder {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TopicSubscription {
    pub topic: WSTopic,
    pub subtopic: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WSMessage {
    pub subscriptions: Vec<TopicSubscription>,
    pub unsubscriptions: Vec<TopicSubscription>,
    pub shared_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WSMessageResponse {
    pub subscriptions: Vec<TopicSubscription>,
    pub shinkai_message: ShinkaiMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WSTopic {
    Inbox,
    SmartInboxes,
}

impl fmt::Display for WSTopic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WSTopic::Inbox => write!(f, "inbox"),
            WSTopic::SmartInboxes => write!(f, "smart_inboxes"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCodeRequest {
    pub permissions: IdentityPermissions,
    pub code_type: RegistrationCodeType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IdentityPermissions {
    Admin,    // can create and delete other profiles
    Standard, // can add / remove devices
    None,     // none of the above
}

impl IdentityPermissions {
    pub fn from_slice(slice: &[u8]) -> Self {
        let s = std::str::from_utf8(slice).unwrap();
        match s {
            "admin" => Self::Admin,
            "standard" => Self::Standard,
            _ => Self::None,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Admin => b"admin",
            Self::Standard => b"standard",
            Self::None => b"none",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "standard" => Some(Self::Standard),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

impl fmt::Display for IdentityPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Standard => write!(f, "standard"),
            Self::None => write!(f, "none"),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum RegistrationCodeType {
    Device(String),
    Profile,
}

impl Serialize for RegistrationCodeType {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            RegistrationCodeType::Device(device_name) => {
                let s = format!("device:{}", device_name);
                serializer.serialize_str(&s)
            }
            RegistrationCodeType::Profile => serializer.serialize_str("profile"),
        }
    }
}

impl<'de> Deserialize<'de> for RegistrationCodeType {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        match parts.get(0) {
            Some(&"device") => {
                let device_name = parts.get(1).unwrap_or(&"main");
                Ok(RegistrationCodeType::Device(device_name.to_string()))
            }
            Some(&"profile") => Ok(RegistrationCodeType::Profile),
            _ => Err(serde::de::Error::custom("Unexpected variant")),
        }
    }
}

impl fmt::Display for RegistrationCodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistrationCodeType::Device(device_name) => write!(f, "device:{}", device_name),
            RegistrationCodeType::Profile => write!(f, "profile"),
        }
    }
}
