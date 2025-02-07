use std::{collections::HashMap, net::SocketAddr};

use async_channel::Sender;
use chrono::{DateTime, Local, Utc};
use ed25519_dalek::VerifyingKey;
use serde_json::{Map, Value};
use shinkai_message_primitives::{
    schemas::{
        coinbase_mpc_config::CoinbaseMPCWalletConfig, crontab::{CronTask, CronTaskAction}, custom_prompt::CustomPrompt, identity::{Identity, StandardIdentity}, job_config::JobConfig, llm_providers::{agent::Agent, serialized_llm_provider::SerializedLLMProvider}, shinkai_name::ShinkaiName, shinkai_subscription::ShinkaiSubscription, shinkai_tool_offering::{ShinkaiToolOffering, UsageTypeInquiry}, shinkai_tools::{CodeLanguage, DynamicToolType}, smart_inbox::{SmartInbox, V2SmartInbox}, tool_router_key::ToolRouterKey, wallet_complementary::{WalletRole, WalletSource}, wallet_mixed::NetworkIdentifier
    }, shinkai_message::{
        shinkai_message::ShinkaiMessage, shinkai_message_schemas::{
            APIAddOllamaModels, APIAvailableSharedItems, APIChangeJobAgentRequest, APIExportSheetPayload, APIImportSheetPayload, APISetSheetUploadedFilesPayload, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveSourceFile, APIVecFsSearchItems, ExportInboxMessagesFormat, IdentityPermissions, JobCreationInfo, JobMessage, RegistrationCodeType, V2ChatMessage
        }
    }, shinkai_utils::job_scope::MinimalJobScope
};

use shinkai_tools_primitives::tools::{
    shinkai_tool::{ShinkaiTool, ShinkaiToolHeader, ShinkaiToolWithAssets}, tool_config::OAuth, tool_playground::ToolPlayground, tool_types::{OperatingSystem, RunnerType}
};
// use crate::{
//     prompts::custom_prompt::CustomPrompt, tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader}, wallet::{
//         coinbase_mpc_wallet::CoinbaseMPCWalletConfig, local_ether_wallet::WalletSource, wallet_manager::WalletRole,
//     }
// };
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::node_api_router::SendResponseBody;

use super::{
    api_v1::api_v1_handlers::APIUseRegistrationCodeSuccessResponse, api_v2::api_v2_handlers_general::InitialRegistrationRequest, node_api_router::{APIError, GetPublicKeysResponse, SendResponseBodyData}
};

pub enum NodeCommand {
    Shutdown,
    // Command to make the node ping all the other nodes it knows about.
    PingAll,
    // Command to request the node's public keys for signing and encryption. The sender will receive the keys.
    GetPublicKeys(Sender<(VerifyingKey, EncryptionPublicKey)>),
    // Command to make the node send a `ShinkaiMessage` in an onionized (i.e., anonymous and encrypted) way.
    SendOnionizedMessage {
        msg: ShinkaiMessage,
        res: async_channel::Sender<Result<SendResponseBodyData, APIError>>,
    },
    GetNodeName {
        res: Sender<String>,
    },
    // Command to request the addresses of all nodes this node is aware of. The sender will receive the list of
    // addresses.
    GetPeers(Sender<Vec<SocketAddr>>),
    // Command to make the node create a registration code through the API. The sender will receive the code.
    APICreateRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    // Command to make the node create a registration code locally. The sender will receive the code.
    LocalCreateRegistrationCode {
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        res: Sender<String>,
    },
    // Command to make the node use a registration code encapsulated in a `ShinkaiMessage`. The sender will receive
    // the result.
    APIUseRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    },
    // Command to request the external profile data associated with a profile name. The sender will receive the data.
    IdentityNameToExternalProfileData {
        name: String,
        res: Sender<StandardIdentity>,
    },
    // Command to fetch the last 'n' messages, where 'n' is defined by `limit`. The sender will receive the messages.
    FetchLastMessages {
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    // Command to request all subidentities that the node manages. The sender will receive the list of subidentities.
    APIGetAllSubidentities {
        res: Sender<Result<Vec<StandardIdentity>, APIError>>,
    },
    GetAllSubidentitiesDevicesAndLLMProviders(Sender<Result<Vec<Identity>, APIError>>),
    APIGetAllInboxesForProfile {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIGetAllSmartInboxesForProfile {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SmartInbox>, APIError>>,
    },
    APIUpdateSmartInboxName {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    APIGetLastMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    APIUpdateJobToFinished {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    GetLastMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIMarkAsReadUpTo {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    MarkAsReadUpTo {
        inbox_name: String,
        up_to_time: String,
        res: Sender<String>,
    },
    APIGetLastUnreadMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    GetLastUnreadMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIGetLastMessagesFromInboxWithBranches {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<Vec<ShinkaiMessage>>, APIError>>,
    },
    GetLastMessagesFromInboxWithBranches {
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<Vec<ShinkaiMessage>>>,
    },
    APIRetryMessageWithInbox {
        inbox_name: String,
        message_hash: String,
        res: Sender<Result<(), APIError>>,
    },
    RetryMessageWithInbox {
        inbox_name: String,
        message_hash: String,
        res: Sender<Result<(), String>>,
    },
    APIAddInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    #[allow(dead_code)]
    APIRemoveInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    RemoveInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    #[allow(dead_code)]
    HasInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<bool>,
    },
    APICreateJob {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    CreateJob {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIJobMessage {
        msg: ShinkaiMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    #[allow(dead_code)]
    JobMessage {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIAddAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddAgent {
        agent: SerializedLLMProvider,
        profile: ShinkaiName,
        res: Sender<String>,
    },
    APIChangeJobAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIAvailableLLMProviders {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedLLMProvider>, APIError>>,
    },
    APIRemoveAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIModifyAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiImportAgent {
        bearer: String,
        url: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExportAgent {
        bearer: String,
        agent_id: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    AvailableLLMProviders {
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedLLMProvider>, String>>,
    },
    APIPrivateDevopsCronList {
        res: Sender<Result<String, APIError>>,
    },
    APIListAllShinkaiTools {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    },
    APISetShinkaiTool {
        tool_router_key: String,
        msg: ShinkaiMessage,
        res: Sender<Result<serde_json::Value, APIError>>,
    },
    APIGetShinkaiTool {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIAddToolkit {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIRemoveToolkit {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIListToolkits {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIChangeNodesName {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    APIIsPristine {
        res: Sender<Result<bool, APIError>>,
    },
    IsPristine {
        res: Sender<bool>,
    },
    APIScanOllamaModels {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    },
    APIAddOllamaModels {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    LocalScanOllamaModels {
        res: Sender<Result<Vec<serde_json::Value>, String>>,
    },
    AddOllamaModels {
        target_profile: ShinkaiName,
        models: Vec<String>,
        res: Sender<Result<(), String>>,
    },
    APIVecFSRetrievePathSimplifiedJson {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrievePathMinimalJson {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrieveVectorResource {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrieveVectorSearchSimplifiedJson {
        msg: ShinkaiMessage,
        #[allow(clippy::complexity)]
        res: Sender<Result<Vec<(String, Vec<String>, f32)>, APIError>>,
    },
    APIConvertFilesAndSaveToFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<Value>, APIError>>,
    },
    APIVecFSCreateFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSMoveItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSCopyItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSMoveFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSCopyFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSDeleteFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSDeleteItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSSearchItems {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIAvailableSharedItems {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIAvailableSharedItemsOpen {
        msg: APIAvailableSharedItems,
        res: Sender<Result<Value, APIError>>,
    },
    APICreateShareableFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUpdateShareableFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUnshareFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APISubscribeToSharedFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUnsubscribe {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIMySubscriptions {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIGetMySubscribers {
        msg: ShinkaiMessage,
        res: Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
    },
    APIGetHttpFreeSubscriptionLinks {
        subscription_profile_path: String,
        res: Sender<Result<Value, APIError>>,
    },
    RetrieveVRKai {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    RetrieveVRPack {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    LocalExtManagerProcessSubscriptionUpdates {
        res: Sender<Result<(), String>>,
    },
    #[allow(dead_code)]
    LocalHttpUploaderProcessSubscriptionUpdates {
        res: Sender<Result<(), String>>,
    },
    #[allow(dead_code)]
    LocalMySubscriptionCallJobMessageProcessing {
        res: Sender<Result<(), String>>,
    },
    #[allow(dead_code)]
    LocalMySubscriptionTriggerHttpDownload {
        res: Sender<Result<(), String>>,
    },
    APIGetLastNotifications {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIGetNotificationsBeforeTimestamp {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APISearchWorkflows {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APISearchShinkaiTool {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIAddWorkflow {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIUpdateWorkflow {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIRemoveWorkflow {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIGetWorkflowInfo {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIListAllWorkflows {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APISetColumn {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIRemoveColumn {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIAddRows {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIRemoveRows {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIUserSheets {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APICreateSheet {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIRemoveSheet {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APISetCellValue {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIGetSheet {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIImportSheet {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIExportSheet {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIUpdateDefaultEmbeddingModel {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUpdateSupportedEmbeddingModels {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    InternalCheckRustToolsInstallation {
        res: Sender<Result<bool, String>>,
    },
    // V2 API
    V2ApiGetPublicKeys {
        res: Sender<Result<GetPublicKeysResponse, APIError>>,
    },
    V2ApiInitialRegistration {
        payload: InitialRegistrationRequest,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    },
    V2ApiAvailableLLMProviders {
        bearer: String,
        res: Sender<Result<Vec<SerializedLLMProvider>, APIError>>,
    },
    V2ApiGetAllSmartInboxes {
        bearer: String,
        limit: Option<usize>,
        offset: Option<String>,
        show_hidden: Option<bool>,
        res: Sender<Result<Vec<V2SmartInbox>, APIError>>,
    },
    V2ApiGetAllSmartInboxesPaginated {
        bearer: String,
        limit: Option<usize>,
        offset: Option<String>,
        show_hidden: Option<bool>,
        res: Sender<Result<serde_json::Value, APIError>>,
    },
    V2ApiUpdateSmartInboxName {
        bearer: String,
        inbox_name: String,
        custom_name: String,
        res: Sender<Result<(), APIError>>,
    },
    V2ApiGetLastMessagesFromInbox {
        bearer: String,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Result<Vec<V2ChatMessage>, APIError>>,
    },
    V2ApiGetLastMessagesFromInboxWithBranches {
        bearer: String,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Result<Vec<Vec<V2ChatMessage>>, APIError>>,
    },
    V2ApiCreateJob {
        bearer: String,
        job_creation_info: JobCreationInfo,
        llm_provider: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiJobMessage {
        bearer: String,
        job_message: JobMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    V2ApiAddMessagesGodMode {
        bearer: String,
        job_id: String,
        messages: Vec<JobMessage>,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiForkJobMessages {
        bearer: String,
        job_id: String,
        message_id: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiRemoveJob {
        bearer: String,
        job_id: String,
        res: Sender<Result<SendResponseBody, APIError>>,
    },
    V2ApiVecFSRetrievePathSimplifiedJson {
        bearer: String,
        payload: APIVecFsRetrievePathSimplifiedJson,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiVecFSRetrieveVectorResource {
        bearer: String,
        path: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiVecFSRetrieveFilesForJob {
        bearer: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiVecFSGetFolderNameForJob {
        bearer: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiVecFSCreateFolder {
        bearer: String,
        payload: APIVecFsCreateFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiMoveItem {
        bearer: String,
        payload: APIVecFsMoveItem,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiCopyItem {
        bearer: String,
        payload: APIVecFsCopyItem,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiMoveFolder {
        bearer: String,
        payload: APIVecFsMoveFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiCopyFolder {
        bearer: String,
        payload: APIVecFsCopyFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiDeleteFolder {
        bearer: String,
        payload: APIVecFsDeleteFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiDeleteItem {
        bearer: String,
        payload: APIVecFsDeleteItem,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiSearchItems {
        bearer: String,
        payload: APIVecFsSearchItems,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiCreateFilesInbox {
        bearer: String, //
        res: Sender<Result<String, APIError>>,
    },
    V2ApiAddFileToInbox {
        bearer: String,
        file_inbox_name: String,
        filename: String,
        file: Vec<u8>,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUploadFileToFolder {
        bearer: String,
        filename: String,
        file: Vec<u8>,
        path: String,
        file_datetime: Option<DateTime<Utc>>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiUploadFileToJob {
        bearer: String,
        job_id: String,
        filename: String,
        file: Vec<u8>,
        file_datetime: Option<DateTime<Utc>>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRetrieveFile {
        bearer: String,
        payload: APIVecFsRetrieveSourceFile,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiSearchWorkflows {
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSearchShinkaiTool {
        bearer: String,
        query: String,
        agent_or_llm: Option<String>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListAllShinkaiTools {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSetShinkaiTool {
        bearer: String,
        tool_key: String,
        payload: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    },
    V2ApiAddShinkaiTool {
        bearer: String,
        shinkai_tool: ShinkaiToolWithAssets,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetShinkaiTool {
        bearer: String,
        payload: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetDefaultEmbeddingModel {
        bearer: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiGetSupportedEmbeddingModels {
        bearer: String,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    V2ApiUpdateDefaultEmbeddingModel {
        bearer: String,
        model_name: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUpdateSupportedEmbeddingModels {
        bearer: String,
        models: Vec<String>,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiAddLlmProvider {
        bearer: String,
        agent: SerializedLLMProvider,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiChangeJobLlmProvider {
        bearer: String,
        payload: APIChangeJobAgentRequest,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUpdateJobConfig {
        bearer: String,
        job_id: String,
        config: JobConfig,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiGetJobConfig {
        bearer: String,
        job_id: String,
        res: Sender<Result<JobConfig, APIError>>,
    },
    V2ApiRemoveLlmProvider {
        bearer: String,
        llm_provider_id: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiModifyLlmProvider {
        bearer: String,
        agent: SerializedLLMProvider,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiChangeNodesName {
        bearer: String,
        new_name: String,
        res: Sender<Result<(), APIError>>,
    },
    V2ApiIsPristine {
        bearer: String,
        res: Sender<Result<bool, APIError>>,
    },
    V2ApiHealthCheck {
        res: Sender<Result<serde_json::Value, APIError>>,
    },
    V2ApiScanOllamaModels {
        bearer: String,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    },
    V2ApiAddOllamaModels {
        bearer: String,
        payload: APIAddOllamaModels,
        res: Sender<Result<(), APIError>>,
    },
    V2ApiGetToolOffering {
        bearer: String,
        tool_key_name: String,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    },
    V2ApiRemoveToolOffering {
        bearer: String,
        tool_key_name: String,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    },
    V2ApiGetAllToolOfferings {
        bearer: String,
        res: Sender<Result<Vec<ShinkaiToolHeader>, APIError>>,
    },
    V2ApiSetToolOffering {
        bearer: String,
        tool_offering: ShinkaiToolOffering,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    },
    V2ApiRestoreLocalEthersWallet {
        bearer: String,
        network: NetworkIdentifier,
        source: WalletSource,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiCreateLocalEthersWallet {
        bearer: String,
        network: NetworkIdentifier,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiCreateCoinbaseMPCWallet {
        bearer: String,
        network: NetworkIdentifier,
        config: Option<CoinbaseMPCWalletConfig>,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRestoreCoinbaseMPCWallet {
        bearer: String,
        network: NetworkIdentifier,
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListWallets {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRequestInvoice {
        bearer: String,
        tool_key_name: String,
        usage: UsageTypeInquiry,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiPayInvoice {
        bearer: String,
        invoice_id: String,
        data_for_tool: Value,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListInvoices {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiAddCustomPrompt {
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    },
    V2ApiDeleteCustomPrompt {
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    },
    V2ApiGetAllCustomPrompts {
        bearer: String,
        res: Sender<Result<Vec<CustomPrompt>, APIError>>,
    },
    V2ApiGetCustomPrompt {
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    },
    V2ApiSearchCustomPrompts {
        bearer: String,
        query: String,
        res: Sender<Result<Vec<CustomPrompt>, APIError>>,
    },
    V2ApiUpdateCustomPrompt {
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    },
    V2ApiStopLLM {
        bearer: String,
        inbox_name: String,
        res: Sender<Result<(), APIError>>,
    },
    V2ApiAddAgent {
        bearer: String,
        agent: Agent,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiRemoveAgent {
        bearer: String,
        agent_id: String,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUpdateAgent {
        bearer: String,
        partial_agent: serde_json::Value,
        res: Sender<Result<Agent, APIError>>,
    },
    V2ApiGetAgent {
        bearer: String,
        agent_id: String,
        res: Sender<Result<Agent, APIError>>,
    },
    V2ApiGetAllAgents {
        bearer: String,
        res: Sender<Result<Vec<Agent>, APIError>>,
    },
    V2ApiRetryMessage {
        bearer: String,
        inbox_name: String,
        message_id: String,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    V2ApiUpdateJobScope {
        bearer: String,
        job_id: String,
        job_scope: MinimalJobScope,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetJobScope {
        bearer: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetToolingLogs {
        bearer: String,
        message_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiImportSheet {
        bearer: String,
        payload: APIImportSheetPayload,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExportSheet {
        bearer: String,
        payload: APIExportSheetPayload,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSetSheetUploadedFiles {
        bearer: String,
        payload: APISetSheetUploadedFilesPayload,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExecuteTool {
        bearer: String,
        tool_router_key: String,
        parameters: Map<String, Value>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        extra_config: Map<String, Value>,
        mounts: Option<Vec<String>>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExecuteCode {
        bearer: String,
        code: String,
        tools: Vec<ToolRouterKey>,
        tool_type: DynamicToolType,
        parameters: Map<String, Value>,
        extra_config: Map<String, Value>,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        mounts: Option<Vec<String>>,
        runner: Option<RunnerType>,
        operating_system: Option<Vec<OperatingSystem>>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGenerateToolDefinitions {
        bearer: String,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGenerateToolFetchQuery {
        bearer: String,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        code: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGenerateToolImplementation {
        bearer: String,
        message: JobMessage,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        post_check: bool,
        raw: bool,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    V2ApiGenerateToolMetadataImplementation {
        bearer: String,
        job_id: String,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExportMessagesFromInbox {
        bearer: String,
        inbox_name: String,
        format: ExportInboxMessagesFormat,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSetPlaygroundTool {
        bearer: String,
        payload: ToolPlayground,
        tool_id: String,
        app_id: String,
        original_tool_key_path: Option<String>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListPlaygroundTools {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRemovePlaygroundTool {
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetPlaygroundTool {
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiToolImplementationUndoTo {
        bearer: String,
        message_hash: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiToolImplementationCodeUpdate {
        bearer: String,
        job_id: String,
        code: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExportTool {
        bearer: String,
        tool_key_path: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    V2ApiPublishTool {
        bearer: String,
        tool_key_path: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiImportTool {
        bearer: String,
        url: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiImportToolZip {
        bearer: String,
        file_data: Vec<u8>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRemoveTool {
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiResolveShinkaiFileProtocol {
        bearer: String,
        shinkai_file_protocol: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    V2ApiAddCronTask {
        bearer: String,
        cron: String,
        action: CronTaskAction,
        name: String,
        description: Option<String>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListAllCronTasks {
        bearer: String,
        res: Sender<Result<Vec<CronTask>, APIError>>,
    },
    V2ApiGetSpecificCronTask {
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Option<CronTask>, APIError>>,
    },
    V2ApiRemoveCronTask {
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetCronTaskLogs {
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiUpdateCronTask {
        bearer: String,
        cron_task_id: i64,
        cron: String,
        action: CronTaskAction,
        name: String,
        description: Option<String>,
        paused: bool,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiForceExecuteCronTask {
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetCronSchedule {
        bearer: String,
        res: Sender<Result<Vec<(CronTask, chrono::DateTime<Local>)>, APIError>>,
    },
    V2ApiTestLlmProvider {
        bearer: String,
        provider: SerializedLLMProvider,
        res: Sender<Result<serde_json::Value, APIError>>,
    },
    V2ApiGetOAuthToken {
        bearer: String,
        connection_name: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSetOAuthToken {
        bearer: String,
        code: String,
        state: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiUploadToolAsset {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        file_data: Vec<u8>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListToolAssets {
        bearer: String,
        tool_id: String,
        app_id: String,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    V2ApiDeleteToolAsset {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiUploadAppFile {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        file_data: Vec<u8>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetAppFile {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    V2ApiUpdateAppFile {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        new_name: Option<String>,
        file_data: Option<Vec<u8>>,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListAppFiles {
        bearer: String,
        tool_id: String,
        app_id: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiDeleteAppFile {
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiImportCronTask {
        bearer: String,
        url: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiExportCronTask {
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    V2ApiSearchFilesByName {
        bearer: String,
        name: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiEnableAllTools {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiDisableAllTools {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiAddRegexPattern {
        bearer: String,
        provider_name: String,
        pattern: String,
        response: String,
        description: Option<String>,
        priority: i32,
        res: Sender<Result<i64, APIError>>,
    },
    V2ApiStoreProxy {
        bearer: String,
        tool_router_key: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiStandAlonePlayground {
        bearer: String,
        code: String,
        metadata: Value,
        assets: Option<Vec<String>>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        parameters: Value,
        config: Value,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiComputeQuestsStatus {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
}
