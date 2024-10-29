use std::{collections::HashMap, net::SocketAddr};

use async_channel::Sender;
use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use serde_json::Value;
use shinkai_message_primitives::{
    schemas::{
        agent::Agent, coinbase_mpc_config::CoinbaseMPCWalletConfig, custom_prompt::CustomPrompt, identity::{Identity, StandardIdentity}, job_config::JobConfig, llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::ShinkaiName, shinkai_subscription::ShinkaiSubscription, shinkai_tool_offering::{ShinkaiToolOffering, UsageTypeInquiry}, smart_inbox::{SmartInbox, V2SmartInbox}, wallet_complementary::{WalletRole, WalletSource}, wallet_mixed::NetworkIdentifier
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAddOllamaModels, APIAvailableSharedItems, APIChangeJobAgentRequest, APIConvertFilesAndSaveToFolder,
            APICreateShareableFolder, APIExportSheetPayload, APIGetLastNotifications, APIGetMySubscribers,
            APIGetNotificationsBeforeTimestamp, APIImportSheetPayload, APISetSheetUploadedFilesPayload, APISetWorkflow,
            APISubscribeToSharedFolder, APIUnshareFolder, APIUnsubscribeToSharedFolder, APIUpdateShareableFolder,
            APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
            APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveSourceFile,
            APIVecFsSearchItems, APIWorkflowKeyname, IdentityPermissions, JobCreationInfo, JobMessage,
            RegistrationCodeType, V2ChatMessage,
        },
    },
    shinkai_utils::job_scope::JobScope,
};

use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
// use crate::{
//     prompts::custom_prompt::CustomPrompt, tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader}, wallet::{
//         coinbase_mpc_wallet::CoinbaseMPCWalletConfig, local_ether_wallet::WalletSource, wallet_manager::WalletRole,
//     }
// };
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::node_api_router::SendResponseBody;

use super::{
    api_v1::api_v1_handlers::APIUseRegistrationCodeSuccessResponse,
    api_v2::api_v2_handlers_general::InitialRegistrationRequest,
    node_api_router::{APIError, GetPublicKeysResponse, SendResponseBodyData},
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
    // Command to request the addresses of all nodes this node is aware of. The sender will receive the list of addresses.
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
    // Command to make the node use a registration code encapsulated in a `ShinkaiMessage`. The sender will receive the result.
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
    APICreateFilesInboxWithSymmetricKey {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIGetFilenamesInInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIAddFileToInboxWithSymmetricKey {
        filename: String,
        file: Vec<u8>,
        public_key: String,
        encrypted_nonce: String,
        res: Sender<Result<String, APIError>>,
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
        res: Sender<Result<Vec<V2SmartInbox>, APIError>>,
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
    V2ApiConvertFilesAndSaveToFolder {
        bearer: String,
        payload: APIConvertFilesAndSaveToFolder,
        res: Sender<Result<Vec<Value>, APIError>>,
    },
    V2ApiDownloadFileFromInbox {
        bearer: String,
        inbox_name: String,
        filename: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    },
    V2ApiListFilesInInbox {
        bearer: String,
        inbox_name: String,
        res: Sender<Result<Vec<String>, APIError>>,
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
        res: Sender<Result<Vec<String>, APIError>>,
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
    V2ApiRetrieveSourceFile {
        bearer: String,
        payload: APIVecFsRetrieveSourceFile,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiAvailableSharedItems {
        bearer: String,
        payload: APIAvailableSharedItems,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiAvailableSharedItemsOpen {
        bearer: String,
        payload: APIAvailableSharedItems,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiCreateShareableFolder {
        bearer: String,
        payload: APICreateShareableFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUpdateShareableFolder {
        bearer: String,
        payload: APIUpdateShareableFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUnshareFolder {
        bearer: String,
        payload: APIUnshareFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiSubscribeToSharedFolder {
        bearer: String,
        payload: APISubscribeToSharedFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiUnsubscribe {
        bearer: String,
        payload: APIUnsubscribeToSharedFolder,
        res: Sender<Result<String, APIError>>,
    },
    V2ApiMySubscriptions {
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetMySubscribers {
        bearer: String,
        payload: APIGetMySubscribers,
        res: Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
    },
    V2ApiGetHttpFreeSubscriptionLinks {
        bearer: String,
        subscription_profile_path: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetLastNotifications {
        bearer: String,
        payload: APIGetLastNotifications,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetNotificationsBeforeTimestamp {
        bearer: String,
        payload: APIGetNotificationsBeforeTimestamp,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSearchWorkflows {
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSearchShinkaiTool {
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiSetWorkflow {
        bearer: String,
        payload: APISetWorkflow,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiRemoveWorkflow {
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiGetWorkflowInfo {
        bearer: String,
        payload: APIWorkflowKeyname,
        res: Sender<Result<Value, APIError>>,
    },
    V2ApiListAllWorkflows {
        bearer: String,
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
        shinkai_tool: ShinkaiTool,
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
    V2ApiRetryMessage {
        bearer: String,
        inbox_name: String,
        message_id: String,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    V2ApiUpdateJobScope {
        bearer: String,
        job_id: String,
        job_scope: JobScope,
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
}
