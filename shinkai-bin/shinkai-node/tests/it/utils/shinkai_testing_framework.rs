use std::path::{Path, PathBuf};

use super::vecfs_test_utils::{
    create_folder, generate_message_with_payload, make_folder_shareable, make_folder_shareable_http_free,
    print_tree_simple, remove_folder, remove_item, retrieve_file_info, show_available_shared_items, upload_file,
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use serde_json::Value;
use shinkai_fs::simple_parser::file_parser_helper::ShinkaiFileParser;
use shinkai_http_api::{node_api_router::APIError, node_commands::NodeCommand};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message_schemas::{
        APIVecFsRetrievePathSimplifiedJson, FileDestinationCredentials, MessageSchemaType,
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

/// Struct to simplify testing by encapsulating common test components.
pub struct ShinkaiTestingFramework {
    pub node_commands_sender: Sender<NodeCommand>,
    pub profile_identity_sk: SigningKey,
    pub profile_encryption_sk: EncryptionStaticKey,
    pub node_encryption_pk: EncryptionPublicKey,
    pub node_identity_name: String,
    pub node_profile_name: String,
    pub bearer_token: String,
}

impl ShinkaiTestingFramework {
    /// Creates a new instance of `ShinkaiTestingFramework`.
    pub fn new(
        node_commands_sender: Sender<NodeCommand>,
        profile_identity_sk: SigningKey,
        profile_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_identity_name: String,
        node_profile_name: String,
        bearer_token: String,
    ) -> Self {
        ShinkaiTestingFramework {
            node_commands_sender,
            profile_identity_sk,
            profile_encryption_sk,
            node_encryption_pk,
            node_identity_name,
            node_profile_name,
            bearer_token,
        }
    }

    /// Create a folder
    pub async fn create_folder(&self, path: &str, folder_name: &str) {
        create_folder(
            &self.node_commands_sender,
            path,
            folder_name,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
        )
        .await
    }

    /// Removes a folder.
    #[allow(dead_code)]
    pub async fn remove_folder(&self, folder_path: &str) {
        remove_folder(
            &self.node_commands_sender,
            folder_path,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
        )
        .await;
    }

    /// Shows available shared items.
    pub async fn show_available_shared_items(&self) {
        show_available_shared_items(
            &self.node_identity_name,
            &self.node_profile_name,
            &self.node_commands_sender,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
        )
        .await;
    }

    /// Makes a folder shareable.
    #[allow(dead_code)]
    pub async fn make_folder_shareable(&self, folder_path: &str) {
        make_folder_shareable(
            &self.node_commands_sender,
            folder_path,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
            None,
        )
        .await;
    }

    /// Makes a folder shareable free (+http).
    pub async fn make_folder_shareable_free_whttp(&self, folder_path: &str, credentials: FileDestinationCredentials) {
        make_folder_shareable_http_free(
            &self.node_commands_sender,
            folder_path,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
            Some(credentials),
        )
        .await;
    }

    /// Uploads a file to a specified folder.
    pub async fn upload_file(&self, folder_name: &str, file_path: &str) {
        let file_path = Path::new(file_path);
        upload_file(
            &self.node_commands_sender,
            folder_name,
            file_path,
            &self.bearer_token,
        )
        .await;
    }

    /// Retrieves file information.
    #[allow(dead_code)]
    pub async fn retrieve_file_info(&self, path: &str, is_simple: bool) -> Value {
        retrieve_file_info(
            &self.node_commands_sender,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
            path,
            is_simple,
        )
        .await
    }

    /// Removes an item.
    #[allow(dead_code)]
    pub async fn remove_item(&self, item_path: &str) {
        remove_item(
            &self.node_commands_sender,
            item_path,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
        )
        .await;
    }

    /// Retrieves the list of subscriptions.
    #[allow(dead_code)]
    pub async fn my_subscriptions(&self) -> Value {
        let msg = ShinkaiMessageBuilder::my_subscriptions(
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            self.node_identity_name.clone(),
            self.node_profile_name.clone(),
            self.node_identity_name.clone(),
            "".to_string(),
        )
        .unwrap();

        // Prepare the response channel
        #[allow(clippy::type_complexity)]
        let (res_send_msg_sender, res_send_msg_receiver): (
            async_channel::Sender<Result<Value, APIError>>,
            async_channel::Receiver<Result<Value, APIError>>,
        ) = async_channel::bounded(1);

        // Send the command
        self.node_commands_sender
            .send(NodeCommand::APIMySubscriptions {
                msg,
                res: res_send_msg_sender,
            })
            .await
            .unwrap();

        res_send_msg_receiver
            .recv()
            .await
            .unwrap()
            .expect("Failed to receive response")
    }

    /// Retrieves simplified path information and optionally prints it based on `should_print`.
    pub async fn retrieve_and_print_path_simplified(&self, path: &str, should_print: bool) -> serde_json::Value {
        let payload = APIVecFsRetrievePathSimplifiedJson {
            path: path.to_string(),
            depth: Some(1),
        };
        let msg = generate_message_with_payload(
            serde_json::to_string(&payload).unwrap(),
            MessageSchemaType::VecFsRetrievePathSimplifiedJson,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            &self.node_identity_name,
            &self.node_profile_name,
            &self.node_identity_name,
            "",
        );

        // Prepare the response channel
        let (res_sender, res_receiver) = async_channel::bounded(1);

        // Send the command
        self.node_commands_sender
            .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
            .await
            .unwrap();
        let response_json = res_receiver.recv().await.unwrap().expect("Failed to receive response");

        if should_print {
            print_tree_simple(response_json.clone());
        }

        response_json
    }

    // Initialize PDF parser, check and download dependencies
    pub async fn initialize_pdfium() {
        #[cfg(target_os = "linux")]
        let os = "linux";

        #[cfg(target_os = "macos")]
        let os = "mac";

        #[cfg(target_os = "windows")]
        let os = "win";

        #[cfg(target_arch = "aarch64")]
        let arch = "arm64";

        #[cfg(target_arch = "x86_64")]
        let arch = "x64";

        let current_directory = std::env::current_dir().unwrap();
        let current_directory = current_directory.iter().collect::<Vec<_>>();
        let project_directory = current_directory
            .iter()
            .take(current_directory.len() - 2)
            .collect::<PathBuf>();
        let pdfium_directory = project_directory.join(format!("shinkai-libs/shinkai-ocr/pdfium/{}-{}", os, arch));
        std::env::set_var("PDFIUM_DYNAMIC_LIB_PATH", pdfium_directory);

        ShinkaiFileParser::initialize_local_file_parser().await.unwrap();
    }
}
