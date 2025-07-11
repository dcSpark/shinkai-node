use crate::schemas::shinkai_tools::DynamicToolType;
use crate::schemas::tool_router_key::ToolRouterKey;
use crate::schemas::{inbox_name::InboxName, llm_providers::serialized_llm_provider::SerializedLLMProvider};
use crate::shinkai_utils::job_scope::MinimalJobScope;
use crate::shinkai_utils::shinkai_path::ShinkaiPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use utoipa::ToSchema;

use super::shinkai_message::{NodeApiData, ShinkaiMessage};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub enum MessageSchemaType {
    JobCreationSchema,
    JobMessageSchema,
    CreateRegistrationCode,
    UseRegistrationCode,
    APIGetMessagesFromInboxRequest,
    APIReadUpToTimeRequest,
    APIAddAgentRequest,
    APIScanOllamaModels,
    APIAddOllamaModels,
    APIRemoveAgentRequest,
    APIModifyAgentRequest,
    APIFinishJob,
    ChangeJobAgentRequest,
    TextContent,
    ChangeNodesName,
    WSMessage,
    FormattedMultiContent, // TODO
    SymmetricKeyExchange,
    EncryptedFileContent,
    Empty,
    VecFsRetrievePathSimplifiedJson,
    VecFsRetrieveVectorResource,
    VecFsRetrieveVRKai,
    VecFsRetrieveVRPack,
    VecFsRetrieveVectorSearchSimplifiedJson,
    VecFsSearchItems,
    VecFsCreateFolder,
    VecFsDeleteFolder,
    VecFsMoveFolder,
    VecFsCopyFolder,
    VecFsCreateItem,
    VecFsMoveItem,
    VecFsCopyItem,
    VecFsDeleteItem,
    AvailableSharedItems,
    AvailableSharedItemsResponse,
    ConvertFilesAndSaveToFolder,
    UpdateLocalProcessingPreference,
    GetProcessingPreference,
    APIRemoveToolkit,
    APIAddToolkit,
    APIListToolkits,
    GetNotificationsBeforeTimestamp,
    GetLastNotifications,
    SearchWorkflows,
    AddWorkflow,
    UpdateWorkflow,
    RemoveWorkflow,
    GetWorkflow,
    ListWorkflows,
    UpdateSupportedEmbeddingModels,
    UpdateDefaultEmbeddingModel,
    SetShinkaiTool,
    ListAllShinkaiTools,
    GetShinkaiTool,
    SearchShinkaiTool,
    InvoiceRequest,
    Invoice,
    PaidInvoice,
    InvoiceResult,
    InvoiceRequestNetworkError,
    AgentNetworkOfferingRequest,
    AgentNetworkOfferingResponse,
}

impl MessageSchemaType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "JobCreationSchema" => Some(Self::JobCreationSchema),
            "JobMessageSchema" => Some(Self::JobMessageSchema),
            "CreateRegistrationCode" => Some(Self::CreateRegistrationCode),
            "UseRegistrationCode" => Some(Self::UseRegistrationCode),
            "APIGetMessagesFromInboxRequest" => Some(Self::APIGetMessagesFromInboxRequest),
            "APIReadUpToTimeRequest" => Some(Self::APIReadUpToTimeRequest),
            "APIAddAgentRequest" => Some(Self::APIAddAgentRequest),
            "APIScanOllamaModels" => Some(Self::APIScanOllamaModels),
            "APIAddOllamaModels" => Some(Self::APIAddOllamaModels),
            "APIRemoveAgentRequest" => Some(Self::APIRemoveAgentRequest),
            "APIModifyAgentRequest" => Some(Self::APIModifyAgentRequest),
            "ChangeJobAgentRequest" => Some(Self::ChangeJobAgentRequest),
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
            "VecFsRetrieveVRKai" => Some(Self::VecFsRetrieveVRKai),
            "VecFsRetrieveVRPack" => Some(Self::VecFsRetrieveVRPack),
            "VecFsRetrieveVectorSearchSimplifiedJson" => Some(Self::VecFsRetrieveVectorSearchSimplifiedJson),
            "VecFsSearchItems" => Some(Self::VecFsSearchItems),
            "VecFsCreateFolder" => Some(Self::VecFsCreateFolder),
            "VecFsDeleteFolder" => Some(Self::VecFsDeleteFolder),
            "VecFsMoveFolder" => Some(Self::VecFsMoveFolder),
            "VecFsCopyFolder" => Some(Self::VecFsCopyFolder),
            "VecFsCreateItem" => Some(Self::VecFsCreateItem),
            "VecFsMoveItem" => Some(Self::VecFsMoveItem),
            "VecFsCopyItem" => Some(Self::VecFsCopyItem),
            "VecFsDeleteItem" => Some(Self::VecFsDeleteItem),
            "AvailableSharedItems" => Some(Self::AvailableSharedItems),
            "AvailableSharedItemsResponse" => Some(Self::AvailableSharedItemsResponse),
            "ConvertFilesAndSaveToFolder" => Some(Self::ConvertFilesAndSaveToFolder),
            "UpdateLocalProcessingPreference" => Some(Self::UpdateLocalProcessingPreference),
            "GetProcessingPreference" => Some(Self::GetProcessingPreference),
            "APIRemoveToolkit" => Some(Self::APIRemoveToolkit),
            "APIAddToolkit" => Some(Self::APIAddToolkit),
            "APIListToolkits" => Some(Self::APIListToolkits),
            "GetNotificationsBeforeTimestamp" => Some(Self::GetNotificationsBeforeTimestamp),
            "GetLastNotifications" => Some(Self::GetLastNotifications),
            "SearchWorkflows" => Some(Self::SearchWorkflows),
            "AddWorkflow" => Some(Self::AddWorkflow),
            "UpdateWorkflow" => Some(Self::UpdateWorkflow),
            "RemoveWorkflow" => Some(Self::RemoveWorkflow),
            "GetWorkflow" => Some(Self::GetWorkflow),
            "ListWorkflows" => Some(Self::ListWorkflows),
            "UpdateSupportedEmbeddingModels" => Some(Self::UpdateSupportedEmbeddingModels),
            "UpdateDefaultEmbeddingModel" => Some(Self::UpdateDefaultEmbeddingModel),
            "SetShinkaiTool" => Some(Self::SetShinkaiTool),
            "ListAllShinkaiTools" => Some(Self::ListAllShinkaiTools),
            "GetShinkaiTool" => Some(Self::GetShinkaiTool),
            "SearchShinkaiTool" => Some(Self::SearchShinkaiTool),
            "InvoiceRequest" => Some(Self::InvoiceRequest),
            "Invoice" => Some(Self::Invoice),
            "PaidInvoice" => Some(Self::PaidInvoice),
            "InvoiceResult" => Some(Self::InvoiceResult),
            "InvoiceRequestNetworkError" => Some(Self::InvoiceRequestNetworkError),
            "AgentNetworkOfferingRequest" => Some(Self::AgentNetworkOfferingRequest),
            "AgentNetworkOfferingResponse" => Some(Self::AgentNetworkOfferingResponse),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::JobCreationSchema => "JobCreationSchema",
            Self::JobMessageSchema => "JobMessageSchema",
            Self::CreateRegistrationCode => "CreateRegistrationCode",
            Self::UseRegistrationCode => "UseRegistrationCode",
            Self::APIGetMessagesFromInboxRequest => "APIGetMessagesFromInboxRequest",
            Self::APIReadUpToTimeRequest => "APIReadUpToTimeRequest",
            Self::APIAddAgentRequest => "APIAddAgentRequest",
            Self::APIScanOllamaModels => "APIScanOllamaModels",
            Self::APIAddOllamaModels => "APIAddOllamaModels",
            Self::APIRemoveAgentRequest => "APIRemoveAgentRequest",
            Self::APIModifyAgentRequest => "APIModifyAgentRequest",
            Self::ChangeJobAgentRequest => "ChangeJobAgentRequest",
            Self::TextContent => "TextContent",
            Self::ChangeNodesName => "ChangeNodesName",
            Self::WSMessage => "WSMessage",
            Self::FormattedMultiContent => "FormattedMultiContent",
            Self::SymmetricKeyExchange => "SymmetricKeyExchange",
            Self::EncryptedFileContent => "FileContent",
            Self::APIFinishJob => "APIFinishJob",
            Self::VecFsRetrievePathSimplifiedJson => "VecFsRetrievePathSimplifiedJson",
            Self::VecFsRetrieveVectorResource => "VecFsRetrieveVectorResource",
            Self::VecFsRetrieveVRKai => "VecFsRetrieveVRKai",
            Self::VecFsRetrieveVRPack => "VecFsRetrieveVRPack",
            Self::VecFsRetrieveVectorSearchSimplifiedJson => "VecFsRetrieveVectorSearchSimplifiedJson",
            Self::VecFsSearchItems => "VecFsSearchItems",
            Self::VecFsCreateFolder => "VecFsCreateFolder",
            Self::VecFsDeleteFolder => "VecFsDeleteFolder",
            Self::VecFsMoveFolder => "VecFsMoveFolder",
            Self::VecFsCopyFolder => "VecFsCopyFolder",
            Self::VecFsCreateItem => "VecFsCreateItem",
            Self::VecFsMoveItem => "VecFsMoveItem",
            Self::VecFsCopyItem => "VecFsCopyItem",
            Self::VecFsDeleteItem => "VecFsDeleteItem",
            Self::AvailableSharedItems => "AvailableSharedItems",
            Self::AvailableSharedItemsResponse => "AvailableSharedItemsResponse",
            Self::ConvertFilesAndSaveToFolder => "ConvertFilesAndSaveToFolder",
            Self::UpdateLocalProcessingPreference => "UpdateLocalProcessingPreference",
            Self::GetProcessingPreference => "GetProcessingPreference",
            Self::APIRemoveToolkit => "APIRemoveToolkit",
            Self::APIAddToolkit => "APIAddToolkit",
            Self::APIListToolkits => "APIListToolkits",
            Self::GetNotificationsBeforeTimestamp => "GetNotificationsBeforeTimestamp",
            Self::GetLastNotifications => "GetLastNotifications",
            Self::SearchWorkflows => "SearchWorkflows",
            Self::AddWorkflow => "AddWorkflow",
            Self::UpdateWorkflow => "UpdateWorkflow",
            Self::RemoveWorkflow => "RemoveWorkflow",
            Self::GetWorkflow => "GetWorkflow",
            Self::ListWorkflows => "ListWorkflows",
            Self::UpdateSupportedEmbeddingModels => "UpdateSupportedEmbeddingModels",
            Self::UpdateDefaultEmbeddingModel => "UpdateDefaultEmbeddingModel",
            Self::SetShinkaiTool => "SetShinkaiTool",
            Self::ListAllShinkaiTools => "ListAllShinkaiTools",
            Self::GetShinkaiTool => "GetShinkaiTool",
            Self::SearchShinkaiTool => "SearchShinkaiTool",
            Self::InvoiceRequest => "InvoiceRequest",
            Self::Invoice => "Invoice",
            Self::PaidInvoice => "PaidInvoice",
            Self::InvoiceResult => "InvoiceResult",
            Self::InvoiceRequestNetworkError => "InvoiceRequestNetworkError",
            Self::AgentNetworkOfferingRequest => "AgentNetworkOfferingRequest",
            Self::AgentNetworkOfferingResponse => "AgentNetworkOfferingResponse",
            Self::Empty => "",
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SymmetricKeyExchange {
    pub shared_secret_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub enum AssociatedUI {
    Playground,
    Cron(String),
    // Add more variants as needed
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct JobCreationInfo {
    pub scope: MinimalJobScope,
    pub is_hidden: Option<bool>,
    pub associated_ui: Option<AssociatedUI>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub enum CallbackAction {
    Job(JobMessage),
    ToolPlayground(ToolPlaygroundAction),
    // ImplementationCheck: (DynamicToolType, available_tools)
    ImplementationCheck(DynamicToolType, Vec<ToolRouterKey>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct JobMessage {
    pub job_id: String,
    pub content: String,
    pub parent: Option<String>,
    // TODO: remove this after checking is safe
    pub sheet_job_data: Option<String>,
    // This is added to force specific tools to be used in the LLM scope
    pub tools: Option<Vec<String>>,
    // Whenever we need to chain actions, we can use this
    pub callback: Option<Box<CallbackAction>>,
    // This is added from the node
    pub metadata: Option<MessageMetadata>,
    // Whenever we want to force the use of a specific tool, we can use this
    pub tool_key: Option<String>,
    // Field that lists associated files of the message
    #[serde(default)]
    pub fs_files_paths: Vec<ShinkaiPath>,
    #[serde(default)]
    pub job_filenames: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MessageMetadata {
    pub tps: Option<String>,
    pub duration_ms: Option<String>,
    pub function_calls: Option<Vec<FunctionCallMetadata>>,
}

// New struct for function call metadata
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct FunctionCallMetadata {
    pub name: String,
    pub arguments: serde_json::Map<String, serde_json::Value>,
    pub tool_router_key: Option<String>,
    pub response: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct V2ChatMessage {
    pub job_message: JobMessage,
    pub sender: String,
    pub sender_subidentity: String,
    pub receiver: String,
    pub receiver_subidentity: String,
    pub node_api_data: NodeApiData,
    pub inbox: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct ToolPlaygroundAction {
    pub tool_router_key: String,
    pub code: String,
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
    pub agent: SerializedLLMProvider,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsRetrievePathSimplifiedJson {
    pub path: String,
    pub depth: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIConvertFilesAndSaveToFolder {
    pub path: String,
    pub file_inbox: String,
    #[schema(value_type = String, format = Date)]
    pub file_datetime: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFSRetrieveVectorResource {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFSRetrieveVRObject {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsRetrieveVectorSearchSimplifiedJson {
    pub search: String,
    pub path: Option<String>,
    pub max_results: Option<usize>,
    pub max_files_to_scan: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsSearchItems {
    pub path: Option<String>,
    pub search: String,
    pub max_results: Option<usize>,
    pub max_files_to_scan: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsCreateFolder {
    pub path: String,
    pub folder_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsDeleteFolder {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsDeleteItem {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsMoveFolder {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsCopyFolder {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsCreateItem {
    pub path: String,
    pub item_name: String,
    pub item_content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsMoveItem {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIVecFsCopyItem {
    pub origin_path: String,
    pub destination_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct APIVecFsRetrieveSourceFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processed_file: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIAvailableSharedItems {
    pub path: String,
    pub streamer_node_name: String,
    pub streamer_profile_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIAddOllamaModels {
    pub models: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct APIGetLastNotifications {
    pub count: usize,
    pub timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct APIGetNotificationsBeforeTimestamp {
    pub timestamp: String,
    pub count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIChangeJobAgentRequest {
    pub job_id: String,
    pub new_agent_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TopicSubscription {
    pub topic: WSTopic,
    pub subtopic: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportInboxMessagesFormat {
    CSV,
    JSON,
    TXT,
}

/// An authenticated WebSocket message that includes a bearer token
/// and the actual WSMessage payload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthenticatedWSMessage {
    pub bearer_auth: String,
    pub message: WSMessage,
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
    Sheet,
    SheetList,
    Widget,
}

impl fmt::Display for WSTopic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WSTopic::Inbox => write!(f, "inbox"),
            WSTopic::SmartInboxes => write!(f, "smart_inboxes"),
            WSTopic::Sheet => write!(f, "sheet"),
            WSTopic::SheetList => write!(f, "sheet_list"),
            WSTopic::Widget => write!(f, "widget"),
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

#[derive(PartialEq, Debug, Clone)]
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
        match parts.first() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_job_message_serialization() {
        let job_message = JobMessage {
            job_id: "test_job".to_string(),
            content: "test content".to_string(),
            parent: Some("parent_id".to_string()),
            sheet_job_data: Some("sheet_data".to_string()),
            tools: Some(vec!["tool1".to_string(), "tool2".to_string()]),
            callback: Some(Box::new(CallbackAction::Job(JobMessage {
                job_id: "callback_job".to_string(),
                content: "callback content".to_string(),
                parent: None,
                sheet_job_data: None,
                tools: None,
                callback: None,
                metadata: None,
                tool_key: None,
                fs_files_paths: vec![],
                job_filenames: vec![],
            }))),
            metadata: Some(MessageMetadata {
                tps: Some("10".to_string()),
                duration_ms: Some("100".to_string()),
                function_calls: Some(vec![FunctionCallMetadata {
                    name: "test_function".to_string(),
                    arguments: {
                        let mut map = serde_json::Map::new();
                        map.insert("arg1".to_string(), json!("value1"));
                        map
                    },
                    tool_router_key: Some("router_key".to_string()),
                    response: Some("function response".to_string()),
                }]),
            }),
            tool_key: Some("specific_tool".to_string()),
            fs_files_paths: vec![],
            job_filenames: vec!["file1.txt".to_string()],
        };

        // Test serialization
        let serialized = serde_json::to_string(&job_message).expect("Failed to serialize JobMessage");

        // Test deserialization
        let deserialized: JobMessage = serde_json::from_str(&serialized).expect("Failed to deserialize JobMessage");

        assert_eq!(job_message, deserialized);
    }

    #[test]
    fn test_job_message_minimal() {
        let minimal_message = JobMessage {
            job_id: "minimal_job".to_string(),
            content: "minimal content".to_string(),
            parent: None,
            sheet_job_data: None,
            tools: None,
            callback: None,
            metadata: None,
            tool_key: None,
            fs_files_paths: vec![],
            job_filenames: vec![],
        };

        let serialized = serde_json::to_string(&minimal_message).expect("Failed to serialize minimal JobMessage");
        let deserialized: JobMessage =
            serde_json::from_str(&serialized).expect("Failed to deserialize minimal JobMessage");

        assert_eq!(minimal_message, deserialized);
    }

    #[test]
    fn test_job_message_specific_json_backward_compatibility() {
        let json_str = r#"{"job_id":"minimal_job","content":"minimal content","parent":null,"sheet_job_data":null,"callback":null,"metadata":null,"tool_key":null,"fs_files_paths":[],"job_filenames":[]}"#;

        let expected = JobMessage {
            job_id: "minimal_job".to_string(),
            content: "minimal content".to_string(),
            parent: None,
            sheet_job_data: None,
            tools: None,
            callback: None,
            metadata: None,
            tool_key: None,
            fs_files_paths: vec![],
            job_filenames: vec![],
        };

        let deserialized: JobMessage =
            serde_json::from_str(json_str).expect("Failed to deserialize specific JSON string");

        assert_eq!(expected, deserialized);
    }
}
