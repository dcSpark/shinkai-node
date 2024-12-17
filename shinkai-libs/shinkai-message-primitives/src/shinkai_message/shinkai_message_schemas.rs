use crate::schemas::sheet::{APIColumnDefinition, ColumnUuid, RowUuid, UuidString};
use crate::schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment};
use crate::schemas::shinkai_tools::DynamicToolType;
use crate::schemas::{inbox_name::InboxName, llm_providers::serialized_llm_provider::SerializedLLMProvider};
use crate::shinkai_utils::job_scope::JobScope;
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
    CreateShareableFolder,
    UpdateShareableFolder,
    UnshareFolder,
    GetMySubscribers,
    ConvertFilesAndSaveToFolder,
    SubscribeToSharedFolder,
    UnsubscribeToSharedFolder,
    SubscribeToSharedFolderResponse,
    UnsubscribeToSharedFolderResponse,
    MySubscriptions,
    SubscriptionRequiresTreeUpdate,
    SubscriptionRequiresTreeUpdateResponse,
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
    UserSheets,
    SetColumn,
    RemoveColumn,
    RemoveSheet,
    CreateEmptySheet,
    SetCellValue,
    GetSheet,
    RemoveRows,
    AddRows,
    ImportSheet,
    ExportSheet,
    SetSheetUploadedFiles,
    SetShinkaiTool,
    ListAllShinkaiTools,
    GetShinkaiTool,
    SearchShinkaiTool,
    InvoiceRequest,
    Invoice,
    PaidInvoice,
    InvoiceResult,
    InvoiceRequestNetworkError,
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
            "CreateShareableFolder" => Some(Self::CreateShareableFolder),
            "UpdateShareableFolder" => Some(Self::UpdateShareableFolder),
            "UnshareFolder" => Some(Self::UnshareFolder),
            "GetMySubscribers" => Some(Self::GetMySubscribers),
            "ConvertFilesAndSaveToFolder" => Some(Self::ConvertFilesAndSaveToFolder),
            "SubscribeToSharedFolder" => Some(Self::SubscribeToSharedFolder),
            "UnsubscribeToSharedFolder" => Some(Self::UnsubscribeToSharedFolder),
            "SubscribeToSharedFolderResponse" => Some(Self::SubscribeToSharedFolderResponse),
            "UnsubscribeToSharedFolderResponse" => Some(Self::UnsubscribeToSharedFolderResponse),
            "MySubscriptions" => Some(Self::MySubscriptions),
            "SubscriptionRequiresTreeUpdate" => Some(Self::SubscriptionRequiresTreeUpdate),
            "SubscriptionRequiresTreeUpdateResponse" => Some(Self::SubscriptionRequiresTreeUpdateResponse),
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
            "UserSheets" => Some(Self::UserSheets),
            "SetColumn" => Some(Self::SetColumn),
            "RemoveColumn" => Some(Self::RemoveColumn),
            "RemoveSheet" => Some(Self::RemoveSheet),
            "CreateEmptySheet" => Some(Self::CreateEmptySheet),
            "SetCellValue" => Some(Self::SetCellValue),
            "GetSheet" => Some(Self::GetSheet),
            "RemoveRows" => Some(Self::RemoveRows),
            "AddRows" => Some(Self::AddRows),
            "ImportSheet" => Some(Self::ImportSheet),
            "ExportSheet" => Some(Self::ExportSheet),
            "SetSheetUploadedFiles" => Some(Self::SetSheetUploadedFiles),
            "SetShinkaiTool" => Some(Self::SetShinkaiTool),
            "ListAllShinkaiTools" => Some(Self::ListAllShinkaiTools),
            "GetShinkaiTool" => Some(Self::GetShinkaiTool),
            "SearchShinkaiTool" => Some(Self::SearchShinkaiTool),
            "InvoiceRequest" => Some(Self::InvoiceRequest),
            "Invoice" => Some(Self::Invoice),
            "PaidInvoice" => Some(Self::PaidInvoice),
            "InvoiceResult" => Some(Self::InvoiceResult),
            "InvoiceRequestNetworkError" => Some(Self::InvoiceRequestNetworkError),
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
            Self::CreateShareableFolder => "CreateShareableFolder",
            Self::UpdateShareableFolder => "UpdateShareableFolder",
            Self::UnshareFolder => "UnshareFolder",
            Self::GetMySubscribers => "GetMySubscribers",
            Self::ConvertFilesAndSaveToFolder => "ConvertFilesAndSaveToFolder",
            Self::SubscribeToSharedFolder => "SubscribeToSharedFolder",
            Self::UnsubscribeToSharedFolder => "UnsubscribeToSharedFolder",
            Self::SubscribeToSharedFolderResponse => "SubscribeToSharedFolderResponse",
            Self::UnsubscribeToSharedFolderResponse => "UnsubscribeToSharedFolderResponse",
            Self::MySubscriptions => "MySubscriptions",
            Self::SubscriptionRequiresTreeUpdate => "SubscriptionRequiresTreeUpdate",
            Self::SubscriptionRequiresTreeUpdateResponse => "SubscriptionRequiresTreeUpdateResponse",
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
            Self::UserSheets => "UserSheets",
            Self::SetColumn => "SetColumn",
            Self::RemoveColumn => "RemoveColumn",
            Self::RemoveSheet => "RemoveSheet",
            Self::CreateEmptySheet => "CreateEmptySheet",
            Self::SetCellValue => "SetCellValue",
            Self::GetSheet => "GetSheet",
            Self::RemoveRows => "RemoveRows",
            Self::AddRows => "AddRows",
            Self::ImportSheet => "ImportSheet",
            Self::ExportSheet => "ExportSheet",
            Self::SetSheetUploadedFiles => "SetSheetUploadedFiles",
            Self::SetShinkaiTool => "SetShinkaiTool",
            Self::ListAllShinkaiTools => "ListAllShinkaiTools",
            Self::GetShinkaiTool => "GetShinkaiTool",
            Self::SearchShinkaiTool => "SearchShinkaiTool",
            Self::InvoiceRequest => "InvoiceRequest",
            Self::Invoice => "Invoice",
            Self::PaidInvoice => "PaidInvoice",
            Self::InvoiceResult => "InvoiceResult",
            Self::InvoiceRequestNetworkError => "InvoiceRequestNetworkError",
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
    Sheet(String),
    Playground,
    // Add more variants as needed
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct JobCreationInfo {
    pub scope: JobScope,
    pub is_hidden: Option<bool>,
    pub associated_ui: Option<AssociatedUI>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub enum CallbackAction {
    Job(JobMessage),
    Sheet(SheetManagerAction),
    ToolPlayground(ToolPlaygroundAction),
    // ImplementationCheck: (DynamicToolType, available_tools)
    ImplementationCheck(DynamicToolType, Vec<String>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct JobMessage {
    pub job_id: String,
    pub content: String,
    pub files_inbox: String,
    pub parent: Option<String>,
    pub sheet_job_data: Option<String>,
    // Whenever we need to chain actions, we can use this
    pub callback: Option<Box<CallbackAction>>,
    // This is added from the node
    pub metadata: Option<MessageMetadata>,
    // Whenever we want to force the use of a specific tool, we can use this
    pub tool_key: Option<String>,
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
pub struct SheetManagerAction {
    pub job_message_next: Option<JobMessage>,
    // TODO: should this be m0re complex and have the actual desired action?
    pub sheet_action: SheetJobAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct ToolPlaygroundAction {
    pub tool_router_key: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct SheetJobAction {
    pub sheet_id: String,
    #[schema(value_type = String)]
    pub row: RowUuid,
    #[schema(value_type = String)]
    pub col: ColumnUuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
pub enum FileDestinationSourceType {
    S3,
    R2,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
pub struct FileDestinationCredentials {
    pub source: FileDestinationSourceType,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint_uri: String,
    pub bucket: String,
}

impl FileDestinationCredentials {
    #[allow(dead_code)]
    pub fn new(
        source: String,
        access_key_id: String,
        secret_access_key: String,
        endpoint_uri: String,
        bucket: String,
    ) -> Self {
        let source_type = match source.as_str() {
            "S3" => FileDestinationSourceType::S3,
            "R2" => FileDestinationSourceType::R2,
            _ => panic!("Unsupported source type"),
        };
        FileDestinationCredentials {
            source: source_type,
            access_key_id,
            secret_access_key,
            endpoint_uri,
            bucket,
        }
    }
}

/// Represents the response for a subscription request, providing details
/// about the subscription status and any errors encountered.
/// Note(Nico): I know things will be much simpler if we added SubscriptionId here
/// but can't trust other nodes, we need to generate those on your side.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SubscriptionGenericResponse {
    // Explanation of what is taking place with this generic response
    pub subscription_details: String,
    /// The overall status of the subscription request.
    pub shared_folder: String,
    /// The overall status of the subscription request.
    pub status: SubscriptionResponseStatus,
    /// Detailed error information, if any errors occurred during the process.
    pub error: Option<SubscriptionError>,
    /// Additional metadata related to the subscription, for extensibility.
    pub metadata: Option<HashMap<String, String>>,
}

/// Represents the status of a subscription request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SubscriptionResponseStatus {
    Success,
    Failure,
    Pending,
    Request,
}

/// Provides structured error information for subscription requests.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SubscriptionError {
    /// A code representing the type of error encountered.
    pub code: String,
    /// A human-readable message describing the error.
    pub message: String,
    /// Additional details or metadata about the error.
    pub details: Option<HashMap<String, String>>,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIAvailableSharedItems {
    pub path: String,
    pub streamer_node_name: String,
    pub streamer_profile_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APISubscribeToSharedFolder {
    pub path: String,
    pub streamer_node_name: String,
    pub streamer_profile_name: String,
    pub payment: SubscriptionPayment,
    pub base_folder: Option<String>,
    pub http_preferred: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIUnsubscribeToSharedFolder {
    pub path: String,
    pub streamer_node_name: String,
    pub streamer_profile_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APICreateShareableFolder {
    pub path: String,
    pub subscription_req: FolderSubscription,
    pub credentials: Option<FileDestinationCredentials>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIUpdateShareableFolder {
    pub path: String,
    pub subscription: FolderSubscription,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIUnshareFolder {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIAddOllamaModels {
    pub models: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct APIGetMySubscribers {
    pub path: String,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct APISetWorkflow {
    pub workflow_raw: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct APISetColumnPayload {
    pub sheet_id: String,
    pub column: APIColumnDefinition,
}

#[derive(Serialize, Deserialize)]
pub struct APIRemoveColumnPayload {
    pub sheet_id: String,
    pub column_id: ColumnUuid,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct APISetCellValuePayload {
    pub sheet_id: String,
    pub row: RowUuid,
    pub col: ColumnUuid,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct APIRemoveRowsPayload {
    pub sheet_id: String,
    pub row_indices: Vec<UuidString>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct APIAddRowsPayload {
    pub sheet_id: String,
    pub number_of_rows: usize,
    pub starting_row: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct APIImportSheetPayload {
    pub sheet_data: SpreadSheetPayload,
    pub sheet_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct APIExportSheetPayload {
    pub sheet_id: String,
    pub file_format: SheetFileFormat,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "content")]
pub enum SpreadSheetPayload {
    CSV(String),
    XLSX(Vec<u8>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum SheetFileFormat {
    CSV,
    XLSX,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct APISetSheetUploadedFilesPayload {
    pub sheet_id: String,
    pub files: HashMap<(UuidString, UuidString), Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct APIWorkflowKeyname {
    pub tool_router_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportInboxMessagesFormat {
    CSV,
    JSON,
    TXT,
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
