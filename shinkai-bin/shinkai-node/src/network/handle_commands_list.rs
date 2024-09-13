use std::sync::Arc;

use crate::{
    lance_db,
    network::{node_commands::NodeCommand, Node},
};

impl Node {
    pub async fn handle_command(&self, command: NodeCommand) {
        match command {
            // Spawn a new task for each command to handle it concurrently

            // NodeCommand::Shutdown => {
            //     shinkai_log(ShinkaiLogOption::Node, ShinkaiLogLevel::Info, "Shutdown command received. Stopping the node.");
            //     // self.db = Arc::new(Mutex::new(ShinkaiDB::new("PLACEHOLDER").expect("Failed to create a temporary database")));
            // },
            NodeCommand::PingAll => {
                let peers_clone = self.peers.clone();
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let listen_address_clone = self.listen_address;
                let proxy_connection_info = self.proxy_connection_info.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Self::ping_all(
                        node_name_clone,
                        encryption_secret_key_clone,
                        identity_secret_key_clone,
                        peers_clone,
                        db_clone,
                        identity_manager_clone,
                        listen_address_clone,
                        proxy_connection_info,
                        ws_manager_trait,
                    )
                    .await;
                });
            }
            NodeCommand::GetPublicKeys(sender) => {
                let identity_public_key = self.identity_public_key;
                let encryption_public_key = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::send_public_keys(identity_public_key, encryption_public_key, sender).await;
                });
            }
            NodeCommand::IdentityNameToExternalProfileData { name, res } => {
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                tokio::spawn(async move {
                    let _ = Self::handle_external_profile_data(identity_manager_clone, name, res).await;
                });
            }
            NodeCommand::SendOnionizedMessage { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let proxy_connection_info = self.proxy_connection_info.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::api_handle_send_onionized_message(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        identity_secret_key_clone,
                        msg,
                        proxy_connection_info,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::FetchLastMessages { limit, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::fetch_and_send_last_messages(db_clone, limit, res).await;
                });
            }
            NodeCommand::GetAllSubidentitiesDevicesAndLLMProviders(res) => {
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                tokio::spawn(async move {
                    let _ =
                        Node::local_get_all_subidentities_devices_and_llm_providers(identity_manager_clone, res).await;
                });
            }
            NodeCommand::LocalCreateRegistrationCode {
                permissions,
                code_type,
                res,
            } => {
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::local_create_and_send_registration_code(db, permissions, code_type, res).await;
                });
            }
            NodeCommand::GetLastMessagesFromInbox {
                inbox_name,
                limit,
                offset_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ =
                        Node::local_get_last_messages_from_inbox(db_clone, inbox_name, limit, offset_key, res).await;
                });
            }
            NodeCommand::MarkAsReadUpTo {
                inbox_name,
                up_to_time,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::local_mark_as_read_up_to(db_clone, inbox_name, up_to_time, res).await;
                });
            }
            NodeCommand::GetLastUnreadMessagesFromInbox {
                inbox_name,
                limit,
                offset,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ =
                        Node::local_get_last_unread_messages_from_inbox(db_clone, inbox_name, limit, offset, res).await;
                });
            }
            NodeCommand::AddInboxPermission {
                inbox_name,
                perm_type,
                identity,
                res,
            } => {
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::local_add_inbox_permission(
                        identity_manager_clone,
                        db_clone,
                        inbox_name,
                        perm_type,
                        identity,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::RemoveInboxPermission {
                inbox_name,
                perm_type,
                identity,
                res,
            } => {
                let identity_manager_clone = Arc::clone(&self.identity_manager);
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::local_remove_inbox_permission(
                        db_clone,
                        identity_manager_clone,
                        inbox_name,
                        perm_type,
                        identity,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::HasInboxPermission {
                inbox_name,
                perm_type,
                identity,
                res,
            } => {
                let identity_manager_clone = self.identity_manager.clone();
                let db_clone = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::has_inbox_permission(
                        identity_manager_clone,
                        db_clone,
                        inbox_name,
                        perm_type,
                        identity,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::CreateJob { shinkai_message, res } => {
                let job_manager_clone = self.job_manager.clone().unwrap();
                let db_clone = self.db.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::local_create_new_job(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        shinkai_message,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::JobMessage { shinkai_message, res } => {
                let job_manager_clone = self.job_manager.clone().unwrap();
                tokio::spawn(async move {
                    let _ = Node::local_job_message(job_manager_clone, shinkai_message, res).await;
                });
            }
            NodeCommand::AddAgent { agent, profile, res } => {
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let db_clone = self.db.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::local_add_llm_provider(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        agent,
                        &profile,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::AvailableLLMProviders { full_profile_name, res } => {
                let db_clone = self.db.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::local_available_llm_providers(db_clone, &node_name_clone, full_profile_name, res).await;
                });
            }
            NodeCommand::LocalScanOllamaModels { res } => {
                tokio::spawn(async move {
                    let _ = Node::local_scan_ollama_models(res).await;
                });
            }
            NodeCommand::AddOllamaModels {
                target_profile,
                models,
                res,
            } => {
                let db_clone = self.db.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::local_add_ollama_models(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        models,
                        target_profile,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APICreateRegistrationCode { msg, res } => {
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_create_and_send_registration_code(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIUseRegistrationCode { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vec_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let first_device_needs_registration_code = self.first_device_needs_registration_code;
                let embedding_generator_clone = Arc::new(self.embedding_generator.clone());
                let encryption_public_key_clone = self.encryption_public_key;
                let identity_public_key_clone = self.identity_public_key;
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let initial_llm_providers_clone = self.initial_llm_providers.clone();
                let job_manager = self.job_manager.clone().unwrap();
                let ws_manager_trait = self.ws_manager_trait.clone();
                let support_embedding_models = self.supported_embedding_models.clone();
                tokio::spawn(async move {
                    let _ = Node::api_handle_registration_code_usage(
                        db_clone,
                        vec_fs_clone,
                        node_name_clone,
                        encryption_secret_key_clone,
                        first_device_needs_registration_code,
                        embedding_generator_clone,
                        identity_manager_clone,
                        job_manager,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        identity_secret_key_clone,
                        initial_llm_providers_clone,
                        msg,
                        ws_manager_trait,
                        support_embedding_models,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIGetAllSubidentities { res } => {
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_all_profiles(identity_manager_clone, res).await;
                });
            }
            NodeCommand::APIGetLastMessagesFromInbox { msg, res } => {
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_last_messages_from_inbox(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res } => {
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_last_unread_messages_from_inbox(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIMarkAsReadUpTo { msg, res } => {
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_mark_as_read_up_to(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APICreateJob { msg, res } => {
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_create_new_job(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        job_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetAllInboxesForProfile { msg, res } => self.api_get_all_inboxes_for_profile(msg, res).await,
            NodeCommand::APIGetAllInboxesForProfile { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_all_inboxes_for_profile(
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAddAgent { msg, res } => self.api_add_agent(msg, res).await,
            NodeCommand::APIAddAgent { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_agent(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        encryption_secret_key_clone,
                        msg,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveAgent { msg, res } => self.api_remove_agent(msg, res).await,
            NodeCommand::APIRemoveAgent { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_agent(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIModifyAgent { msg, res } => self.api_modify_agent(msg, res).await,
            NodeCommand::APIModifyAgent { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_modify_agent(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIJobMessage { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                tokio::spawn(async move {
                    let _ = Node::api_job_message(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        job_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIChangeJobAgent { msg, res } => self.api_change_job_agent(msg, res).await,
            NodeCommand::APIChangeJobAgent { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_change_job_agent(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAvailableLLMProviders { msg, res } => self.api_available_llm_providers(msg, res).await,
            NodeCommand::APIAvailableLLMProviders { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_available_llm_providers(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res } => self.api_create_files_inbox_with_symmetric_key(msg, res).await,
            NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::api_create_files_inbox_with_symmetric_key(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetFilenamesInInbox { msg, res } => self.api_get_filenames_in_inbox(msg, res).await,
            NodeCommand::APIGetFilenamesInInbox { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::api_get_filenames_in_inbox(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAddFileToInboxWithSymmetricKey { filename, file, public_key, encrypted_nonce, res } => self.api_add_file_to_inbox_with_symmetric_key(filename, file, public_key, encrypted_nonce, res).await,
            NodeCommand::APIAddFileToInboxWithSymmetricKey {
                filename,
                file,
                public_key,
                encrypted_nonce,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_file_to_inbox_with_symmetric_key(
                        db_clone,
                        vector_fs_clone,
                        filename,
                        file,
                        public_key,
                        encrypted_nonce,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetAllSmartInboxesForProfile { msg, res } => self.api_get_all_smart_inboxes_for_profile(msg, res).await,
            NodeCommand::APIGetAllSmartInboxesForProfile { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_all_smart_inboxes_for_profile(
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateSmartInboxName { msg, res } => self.api_update_smart_inbox_name(msg, res).await,
            NodeCommand::APIUpdateSmartInboxName { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_update_smart_inbox_name(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateJobToFinished { msg, res } => self.api_update_job_to_finished(msg, res).await,
            NodeCommand::APIUpdateJobToFinished { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_update_job_to_finished(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIPrivateDevopsCronList { res } => self.api_private_devops_cron_list(res).await,
            NodeCommand::APIPrivateDevopsCronList { res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::api_private_devops_cron_list(db_clone, node_name_clone, res).await;
                });
            }
            // NodeCommand::APIAddToolkit { msg, res } => self.api_add_toolkit(msg, res).await,
            NodeCommand::APIAddToolkit { msg, res } => {
                let lance_db = self.lance_db.clone();
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_toolkit(
                        lance_db,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIListAllShinkaiTools { msg, res } => {
                let lance_db = self.lance_db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_list_all_shinkai_tools(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APISetShinkaiTool {
                tool_router_key,
                msg,
                res,
            } => {
                let lance_db = self.lance_db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_set_shinkai_tool(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        tool_router_key,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIGetShinkaiTool { msg, res } => {
                let lance_db = self.lance_db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_shinkai_tool(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveToolkit { msg, res } => self.api_remove_toolkit(msg, res).await,
            NodeCommand::APIRemoveToolkit { msg, res } => {
                let lance_db = self.lance_db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_toolkit(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIListToolkits { msg, res } => self.api_list_toolkits(msg, res).await,
            NodeCommand::APIListToolkits { msg, res } => {
                let lance_db = self.lance_db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_list_toolkits(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APISetColumn { msg: ShinkaiMessage, res: Sender<Result<Value, APIError>> },
            NodeCommand::APISetColumn { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_set_column(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveColumn { msg: ShinkaiMessage, res: Sender<Result<Value, APIError>> },
            NodeCommand::APIRemoveColumn { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_column(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAddRows { msg: ShinkaiMessage, res: Sender<Result<Value, APIError>> },
            NodeCommand::APIAddRows { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_rows(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveRows { msg: ShinkaiMessage, res: Sender<Result<Value, APIError>> },
            NodeCommand::APIRemoveRows { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_rows(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUserSheets { msg: ShinkaiMessage, res: Sender<Result<Value, APIError>> },
            NodeCommand::APIUserSheets { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_user_sheets(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APICreateSheet { msg, res }
            NodeCommand::APICreateSheet { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_create_empty_sheet(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveSheet { msg, res }
            NodeCommand::APIRemoveSheet { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_sheet(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APISetCellValue { msg, res }
            NodeCommand::APISetCellValue { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_set_cell_value(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetSheet { msg, res }
            NodeCommand::APIGetSheet { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let sheet_manager = self.sheet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_sheet(
                        sheet_manager,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIScanOllamaModels { msg, res } => self.api_scan_ollama_models(msg, res).await,
            NodeCommand::APIScanOllamaModels { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_scan_ollama_models(
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAddOllamaModels { msg, res } => self.api_add_ollama_models(msg, res).await,
            NodeCommand::APIAddOllamaModels { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_ollama_models(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        encryption_secret_key_clone,
                        msg,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIChangeNodesName { msg, res } => self.api_change_nodes_name(msg, res).await,
            NodeCommand::APIChangeNodesName { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let identity_public_key_clone = self.identity_public_key;
                let secret_file_path = self.secrets_file_path.clone();
                tokio::spawn(async move {
                    let _ = Node::api_change_nodes_name(
                        secret_file_path.as_str(),
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIIsPristine { res } => self.api_is_pristine(res).await,
            NodeCommand::APIIsPristine { res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Self::api_is_pristine(db_clone, res).await;
                });
            }
            // NodeCommand::IsPristine { res } => self.local_is_pristine(res).await,
            NodeCommand::IsPristine { res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Self::local_is_pristine(db_clone, res).await;
                });
            }
            // NodeCommand::GetNodeName { res: Sender<String> },
            NodeCommand::GetNodeName { res } => {
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = res.send(node_name.node_name).await;
                });
            }
            // NodeCommand::APIGetLastMessagesFromInboxWithBranches { msg, res } => self.api_get_last_messages_from_inbox_with_branches(msg, res).await,
            NodeCommand::APIGetLastMessagesFromInboxWithBranches { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_last_messages_from_inbox_with_branches(
                        encryption_secret_key_clone,
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::GetLastMessagesFromInboxWithBranches { inbox_name, limit, offset_key, res } => self.local_get_last_messages_from_inbox_with_branches(inbox_name, limit, offset_key, res).await,
            NodeCommand::GetLastMessagesFromInboxWithBranches {
                inbox_name,
                limit,
                offset_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::local_get_last_messages_from_inbox_with_branches(
                        db_clone, inbox_name, limit, offset_key, res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res } => self.api_vec_fs_retrieve_path_simplified_json(msg, res).await,
            NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_retrieve_path_simplified_json(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        ext_subscription_manager_clone,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res } => self.api_vec_fs_retrieve_path_minimal_json(msg, res).await,
            NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_retrieve_path_minimal_json(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        ext_subscription_manager_clone,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIConvertFilesAndSaveToFolder { msg, res } => self.api_convert_files_and_save_to_folder(msg, res).await,
            NodeCommand::APIConvertFilesAndSaveToFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let embedding_generator_clone = self.embedding_generator.clone();
                let unstructured_api_clone = self.unstructured_api.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_convert_files_and_save_to_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        Arc::new(embedding_generator_clone),
                        Arc::new(unstructured_api_clone),
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res } => self.api_vec_fs_retrieve_vector_search_simplified_json(msg, res).await,
            NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_retrieve_vector_search_simplified_json(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSSearchItems { msg, res } => self.api_vec_fs_search_items(msg, res).await,
            NodeCommand::APIVecFSSearchItems { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_search_items(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSCreateFolder { msg, res } => self.api_vec_fs_create_folder(msg, res).await,
            NodeCommand::APIVecFSCreateFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_create_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSMoveItem { msg, res } => self.api_vec_fs_move_item(msg, res).await,
            NodeCommand::APIVecFSMoveItem { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_move_item(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSCopyItem { msg, res } => self.api_vec_fs_copy_item(msg, res).await,
            NodeCommand::APIVecFSCopyItem { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_copy_item(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSMoveFolder { msg, res } => self.api_vec_fs_move_folder(msg, res).await,
            NodeCommand::APIVecFSMoveFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_move_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSCopyFolder { msg, res } => self.api_vec_fs_copy_folder(msg, res).await,
            NodeCommand::APIVecFSCopyFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_copy_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSRetrieveVectorResource { msg, res } => self.api_vec_fs_retrieve_vector_resource(msg, res).await,
            NodeCommand::APIVecFSRetrieveVectorResource { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_retrieve_vector_resource(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSDeleteFolder { msg, res } => self.api_vec_fs_delete_folder(msg, res).await,
            NodeCommand::APIVecFSDeleteFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_delete_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIVecFSDeleteItem { msg, res } => self.api_vec_fs_delete_item(msg, res).await,
            NodeCommand::APIVecFSDeleteItem { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_vec_fs_delete_item(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAvailableSharedItems { msg, res } => self.api_subscription_available_shared_items(msg, res).await,
            NodeCommand::APIAvailableSharedItems { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_available_shared_items(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        ext_subscription_manager_clone,
                        my_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAvailableSharedItemsOpen { msg, res } => self.api_subscription_available_shared_items_open(msg, res).await,
            NodeCommand::APIAvailableSharedItemsOpen { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_available_shared_items_open(
                        node_name_clone,
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APICreateShareableFolder { msg, res } => self.api_subscription_create_shareable_folder(msg, res).await,
            NodeCommand::APICreateShareableFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_create_shareable_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateShareableFolder { msg, res } => self.api_subscription_update_shareable_folder(msg, res).await,
            NodeCommand::APIUpdateShareableFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_update_shareable_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUnshareFolder { msg, res } => self.api_subscription_unshare_folder(msg, res).await,
            NodeCommand::APIUnshareFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_unshare_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APISubscribeToSharedFolder { msg, res } => self.api_subscription_subscribe_to_shared_folder(msg, res).await,
            NodeCommand::APISubscribeToSharedFolder { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_subscribe_to_shared_folder(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        my_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIMySubscriptions { msg, res } => self.api_subscription_my_subscriptions(msg, res).await,
            NodeCommand::APIMySubscriptions { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_subscription_my_subscriptions(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUnsubscribe { msg, res } => self.api_unsubscribe_my_subscriptions(msg, res).await,
            NodeCommand::APIUnsubscribe { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_unsubscribe_my_subscriptions(
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        my_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetMySubscribers { msg, res } => self.api_get_my_subscribers(msg, res).await,
            NodeCommand::APIGetMySubscribers { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_my_subscribers(
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        ext_subscription_manager_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetHttpFreeSubscriptionLinks { subscription_id: ShinkaiMessage, res: Sender<Result<Value, APIError>>, },
            NodeCommand::APIGetHttpFreeSubscriptionLinks {
                subscription_profile_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_http_free_subscription_links(
                        db_clone,
                        node_name_clone,
                        ext_subscription_manager_clone,
                        subscription_profile_path,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::RetrieveVRKai { msg, res } => self.retrieve_vr_kai(msg, res).await,
            NodeCommand::RetrieveVRKai { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::retrieve_vr_kai(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::RetrieveVRPack { msg, res } => self.retrieve_vr_pack(msg, res).await,
            NodeCommand::RetrieveVRPack { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::retrieve_vr_pack(
                        db_clone,
                        vector_fs_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::LocalExtManagerProcessSubscriptionUpdates { res } => self.local_ext_manager_process_subscription_updates(res).await,
            NodeCommand::LocalExtManagerProcessSubscriptionUpdates { res } => {
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::local_ext_manager_process_subscription_updates(ext_subscription_manager_clone, res).await;
                });
            }
            // NodeCommand::LocalHttpUploaderProcessSubscriptionUpdates { res } => self.local_http_uploader_process_subscription_updates(res).await,
            NodeCommand::LocalHttpUploaderProcessSubscriptionUpdates { res } => {
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::local_http_uploader_process_subscription_updates(ext_subscription_manager_clone, res)
                        .await;
                });
            }
            // NodeCommand:: { res } => self.local_mysubscription_manager_process_download_updates(res).await,
            NodeCommand::LocalMySubscriptionCallJobMessageProcessing { res } => {
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::local_mysubscription_manager_process_download_updates(my_subscription_manager_clone, res)
                            .await;
                });
            }
            // NodeCommand:: { res } => self.local_mysubscription_trigger_http_download(res).await,
            NodeCommand::LocalMySubscriptionTriggerHttpDownload { res } => {
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::local_mysubscription_trigger_http_download(my_subscription_manager_clone, res).await;
                });
            }
            // NodeCommand:: { res } => self.get_last_notifications(res).await,
            NodeCommand::APIGetLastNotifications { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_last_notifications(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand:: { res } => self.get_notifications_before_timestamp(res).await,
            NodeCommand::APIGetNotificationsBeforeTimestamp { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_notifications_before_timestamp(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // Add these inside the match command block:
            NodeCommand::APIGetLocalProcessingPreference { msg, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_local_processing_preference(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APIUpdateLocalProcessingPreference { preference, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_update_local_processing_preference(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        preference,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APISearchWorkflows { msg, res } => self.api_search_workflows(msg, res).await,
            NodeCommand::APISearchWorkflows { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let tool_router_clone = self.tool_router.clone();
                let embedding_generator_clone = Arc::new(self.embedding_generator.clone());
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::api_search_workflows(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        tool_router_clone,
                        msg,
                        embedding_generator_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::APISearchShinkaiTool { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let tool_router_clone = self.tool_router.clone();
                let embedding_generator_clone = Arc::new(self.embedding_generator.clone());
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::api_search_shinkai_tool(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        tool_router_clone,
                        msg,
                        embedding_generator_clone,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIAddWorkflow { msg, res } => self.api_add_workflow(msg, res).await,
            NodeCommand::APIAddWorkflow { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::api_add_workflow(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateWorkflow { msg, res } => self.api_update_workflow(msg, res).await,
            NodeCommand::APIUpdateWorkflow { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    // Note: yes it's the same as above
                    let _ = Node::api_add_workflow(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIRemoveWorkflow { msg, res } => self.api_remove_workflow(msg, res).await,
            NodeCommand::APIRemoveWorkflow { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::api_remove_workflow(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIGetWorkflowInfo { msg, res } => self.api_get_workflow_info(msg, res).await,
            NodeCommand::APIGetWorkflowInfo { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::api_get_workflow_info(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIListAllWorkflows { msg, res } => self.api_list_all_workflows(msg, res).await,
            NodeCommand::APIListAllWorkflows { msg, res } => {
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::api_list_all_workflows(
                        lance_db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateDefaultEmbeddingModel { msg, res } => self.api_update_default_embedding_model(msg, res).await,
            NodeCommand::APIUpdateDefaultEmbeddingModel { msg, res } => {
                let db = self.db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_update_default_embedding_model(
                        db,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            // NodeCommand::APIUpdateSupportedEmbeddingModels { msg, res } => self.api_update_supported_embedding_models(msg, res).await,
            NodeCommand::APIUpdateSupportedEmbeddingModels { msg, res } => {
                let db = self.db.clone();
                let vector_fs = self.vector_fs.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::api_update_supported_embedding_models(
                        db,
                        vector_fs,
                        node_name_clone,
                        identity_manager_clone,
                        encryption_secret_key_clone,
                        msg,
                        res,
                    )
                    .await;
                });
            }
            //
            // V2 API
            //
            NodeCommand::V2ApiGetPublicKeys { res: sender } => {
                let identity_public_key = self.identity_public_key;
                let encryption_public_key = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::v2_send_public_keys(identity_public_key, encryption_public_key, sender).await;
                });
            }
            NodeCommand::V2ApiInitialRegistration { payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let first_device_needs_registration_code = self.first_device_needs_registration_code;
                let embedding_generator_clone = Arc::new(self.embedding_generator.clone());
                let encryption_public_key_clone = self.encryption_public_key;
                let identity_public_key_clone = self.identity_public_key;
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let initial_llm_providers_clone = self.initial_llm_providers.clone();
                let job_manager = self.job_manager.clone().unwrap();
                let ws_manager_trait = self.ws_manager_trait.clone();
                let supported_embedding_models = self.supported_embedding_models.clone();

                tokio::spawn(async move {
                    let _ = Node::v2_handle_initial_registration(
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        payload,
                        res,
                        vector_fs_clone,
                        first_device_needs_registration_code,
                        embedding_generator_clone,
                        job_manager,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        identity_secret_key_clone,
                        initial_llm_providers_clone,
                        ws_manager_trait,
                        supported_embedding_models,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiCreateJob {
                bearer,
                job_creation_info,
                llm_provider,
                res,
            } => {
                let job_manager_clone = self.job_manager.clone().unwrap();
                let node_name_clone = self.node_name.clone();
                let db_clone = self.db.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let signing_secret_key_clone = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_create_new_job(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        bearer,
                        job_creation_info,
                        llm_provider,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiJobMessage {
                bearer,
                job_message,
                res,
            } => {
                let job_manager_clone = self.job_manager.clone().unwrap();
                let node_name_clone = self.node_name.clone();
                let db_clone = self.db.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let signing_secret_key_clone = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_job_message(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        bearer,
                        job_message,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetLastMessagesFromInbox {
                bearer,
                inbox_name,
                limit,
                offset_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_get_last_messages_from_inbox(db_clone, bearer, inbox_name, limit, offset_key, res)
                        .await;
                });
            }
            NodeCommand::V2ApiGetLastMessagesFromInboxWithBranches {
                bearer,
                inbox_name,
                limit,
                offset_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_get_last_messages_from_inbox_with_branches(
                        db_clone, bearer, inbox_name, limit, offset_key, res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetAllSmartInboxes { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_get_all_smart_inboxes(db_clone, identity_manager_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiAvailableLLMProviders { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_get_available_llm_providers(db_clone, node_name_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiVecFSRetrievePathSimplifiedJson { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_vec_fs_retrieve_path_simplified_json(
                        db_clone,
                        vector_fs_clone,
                        identity_manager_clone,
                        payload,
                        ext_subscription_manager_clone,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiConvertFilesAndSaveToFolder { payload, bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let embedding_generator_clone = self.embedding_generator.clone();
                let unstructured_api_clone = self.unstructured_api.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_convert_files_and_save_to_folder(
                        db_clone,
                        vector_fs_clone,
                        identity_manager_clone,
                        payload,
                        Arc::new(embedding_generator_clone),
                        Arc::new(unstructured_api_clone),
                        ext_subscription_manager_clone,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiVecFSCreateFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_create_folder(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }
            NodeCommand::V2ApiMoveItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_move_item(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                        .await;
                });
            }

            NodeCommand::V2ApiCopyItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_copy_item(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                        .await;
                });
            }

            NodeCommand::V2ApiMoveFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_move_folder(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }

            NodeCommand::V2ApiCopyFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_copy_folder(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }

            NodeCommand::V2ApiDeleteFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_delete_folder(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }

            NodeCommand::V2ApiDeleteItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_delete_item(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }

            NodeCommand::V2ApiSearchItems { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_search_items(db_clone, vector_fs_clone, identity_manager_clone, payload, bearer, res)
                            .await;
                });
            }
            NodeCommand::V2ApiVecFSRetrieveVectorResource { bearer, path, res } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_retrieve_vector_resource(
                        db_clone,
                        vector_fs_clone,
                        identity_manager_clone,
                        path,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUpdateSmartInboxName {
                bearer,
                inbox_name,
                custom_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_update_smart_inbox_name(db_clone, bearer, inbox_name, custom_name, res).await;
                });
            }
            NodeCommand::V2ApiCreateFilesInbox { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_create_files_inbox(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiAddFileToInbox {
                file_inbox_name,
                filename,
                file,
                bearer,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_add_file_to_inbox(
                        db_clone,
                        vector_fs_clone,
                        file_inbox_name,
                        filename,
                        file,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUploadFileToFolder {
                bearer,
                filename,
                file,
                path,
                file_datetime,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let embedding_generator_clone = self.embedding_generator.clone();
                let unstructured_api_clone = self.unstructured_api.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_upload_file_to_folder(
                        db_clone,
                        vector_fs_clone,
                        identity_manager_clone,
                        Arc::new(embedding_generator_clone),
                        Arc::new(unstructured_api_clone),
                        ext_subscription_manager_clone,
                        bearer,
                        filename,
                        file,
                        path,
                        file_datetime,
                        res,
                    )
                    .await;
                });
            }
            // New Code
            NodeCommand::V2ApiAvailableSharedItems { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_available_shared_items(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        ext_subscription_manager_clone,
                        my_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAvailableSharedItemsOpen { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_available_shared_items_open(
                        db_clone,
                        node_name_clone,
                        ext_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiCreateShareableFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_create_shareable_folder(
                        db_clone,
                        identity_manager_clone,
                        ext_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUpdateShareableFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_shareable_folder(
                        db_clone,
                        identity_manager_clone,
                        ext_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUnshareFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_unshare_folder(
                        db_clone,
                        identity_manager_clone,
                        ext_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiSubscribeToSharedFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_subscribe_to_shared_folder(
                        db_clone,
                        identity_manager_clone,
                        my_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUnsubscribe { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let my_subscription_manager_clone = self.my_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_unsubscribe(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        my_subscription_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiMySubscriptions { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_my_subscriptions(db_clone, node_name_clone, identity_manager_clone, bearer, res)
                            .await;
                });
            }
            NodeCommand::V2ApiGetMySubscribers { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_get_my_subscribers(db_clone, ext_subscription_manager_clone, bearer, payload, res)
                            .await;
                });
            }
            NodeCommand::V2ApiGetHttpFreeSubscriptionLinks {
                bearer,
                subscription_profile_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_http_free_subscription_links(
                        db_clone,
                        ext_subscription_manager_clone,
                        bearer,
                        subscription_profile_path,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetLastNotifications { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_last_notifications(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetNotificationsBeforeTimestamp { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_notifications_before_timestamp(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        bearer,
                        payload,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetLocalProcessingPreference { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_local_processing_preference(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiUpdateLocalProcessingPreference {
                bearer,
                preference,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_local_processing_preference(db_clone, bearer, preference, res).await;
                });
            }
            NodeCommand::V2ApiSearchWorkflows { bearer, query, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_search_workflows(db_clone, lance_db, bearer, query, res).await;
                });
            }
            NodeCommand::V2ApiSearchShinkaiTool { bearer, query, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_search_shinkai_tool(db_clone, lance_db, bearer, query, res).await;
                });
            }
            NodeCommand::V2ApiSetWorkflow { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_workflow(db_clone, lance_db, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiRemoveWorkflow { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_workflow(db_clone, lance_db, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiGetWorkflowInfo { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_workflow_info(db_clone, lance_db, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiListAllWorkflows { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_workflows(db_clone, lance_db, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetDefaultEmbeddingModel { bearer, res } => {
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_default_embedding_model(db, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetSupportedEmbeddingModels { bearer, res } => {
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_supported_embedding_models(db, bearer, res).await;
                });
            }
            NodeCommand::V2ApiUpdateDefaultEmbeddingModel {
                bearer,
                model_name,
                res,
            } => {
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_default_embedding_model(db, bearer, model_name, res).await;
                });
            }
            NodeCommand::V2ApiUpdateSupportedEmbeddingModels { bearer, models, res } => {
                let db = self.db.clone();
                let vector_fs = self.vector_fs.clone();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_supported_embedding_models(
                        db,
                        vector_fs,
                        identity_manager_clone,
                        bearer,
                        models,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAddLlmProvider { bearer, agent, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_llm_provider(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        bearer,
                        agent,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiChangeJobLlmProvider { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_change_job_llm_provider(db_clone, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiUpdateJobConfig {
                bearer,
                job_id,
                config,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_job_config(db_clone, bearer, job_id, config, res).await;
                });
            }
            NodeCommand::V2ApiGetJobConfig { bearer, job_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_job_config(db_clone, bearer, job_id, res).await;
                });
            }
            NodeCommand::V2ApiRemoveLlmProvider {
                bearer,
                llm_provider_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_llm_provider(
                        db_clone,
                        identity_manager_clone,
                        bearer,
                        llm_provider_id,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiModifyLlmProvider { bearer, agent, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_modify_llm_provider(db_clone, identity_manager_clone, bearer, agent, res).await;
                });
            }
            NodeCommand::V2ApiChangeNodesName { bearer, new_name, res } => {
                let db_clone = Arc::clone(&self.db);
                let secret_file_path = self.secrets_file_path.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_public_key_clone = self.encryption_public_key.clone();
                let identity_public_key_clone = self.identity_public_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_change_nodes_name(
                        bearer,
                        db_clone,
                        &secret_file_path,
                        identity_manager_clone,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        new_name,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiIsPristine { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_is_pristine(bearer, db_clone, res).await;
                });
            }
            NodeCommand::V2ApiScanOllamaModels { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_scan_ollama_models(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiListAllShinkaiTools { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_shinkai_tools(db_clone, lance_db, bearer, res).await;
                });
            }
            NodeCommand::V2ApiSetShinkaiTool {
                bearer,
                tool_key,
                payload,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_shinkai_tool(db_clone, lance_db, bearer, tool_key, payload, res).await;
                });
            }
            NodeCommand::V2ApiAddShinkaiTool {
                bearer,
                shinkai_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_shinkai_tool(db_clone, lance_db, bearer, shinkai_tool, res).await;
                });
            }
            NodeCommand::V2ApiGetShinkaiTool { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_shinkai_tool(db_clone, lance_db, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiAddOllamaModels { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_ollama_models(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        bearer,
                        payload,
                        ws_manager_trait,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiDownloadFileFromInbox {
                bearer,
                inbox_name,
                filename,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_download_file_from_inbox(
                        db_clone,
                        vector_fs_clone,
                        bearer,
                        inbox_name,
                        filename,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListFilesInInbox {
                bearer,
                inbox_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let vector_fs_clone = self.vector_fs.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_files_in_inbox(db_clone, vector_fs_clone, bearer, inbox_name, res).await;
                });
            }
            NodeCommand::V2ApiGetToolOffering {
                bearer,
                tool_key_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_tool_offering(db_clone, bearer, tool_key_name, res).await;
                });
            }
            NodeCommand::V2ApiRemoveToolOffering {
                bearer,
                tool_key_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_tool_offering(db_clone, bearer, tool_key_name, res).await;
                });
            }
            NodeCommand::V2ApiGetAllToolOfferings { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_tool_offering(db_clone, lance_db, bearer, res).await;
                });
            }
            NodeCommand::V2ApiSetToolOffering {
                bearer,
                tool_offering,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_tool_offering(db_clone, lance_db, bearer, tool_offering, res).await;
                });
            }
            NodeCommand::V2ApiRestoreLocalEthersWallet {
                bearer,
                network,
                source,
                role,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let wallet_manager_clone = self.wallet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_restore_local_ethers_wallet(
                        db_clone,
                        wallet_manager_clone,
                        bearer,
                        network,
                        source,
                        role,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiCreateLocalEthersWallet {
                bearer,
                network,
                role,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let wallet_manager_clone = self.wallet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_create_local_ethers_wallet(
                        db_clone,
                        wallet_manager_clone,
                        bearer,
                        network,
                        role,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiRestoreCoinbaseMPCWallet {
                bearer,
                network,
                config,
                wallet_id,
                role,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                let wallet_manager_clone = self.wallet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_restore_coinbase_mpc_wallet(
                        db_clone,
                        lance_db,
                        wallet_manager_clone,
                        bearer,
                        network,
                        config,
                        wallet_id,
                        role,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiCreateCoinbaseMPCWallet {
                bearer,
                network,
                config,
                role,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db = self.lance_db.clone();
                let wallet_manager_clone = self.wallet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_create_coinbase_mpc_wallet(
                        db_clone,
                        lance_db,
                        wallet_manager_clone,
                        bearer,
                        network,
                        config,
                        role,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListWallets { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let wallet_manager_clone = self.wallet_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_wallets(db_clone, wallet_manager_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiRequestInvoice {
                bearer,
                tool_key_name,
                usage,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let my_agent_payments_manager_clone = self.my_agent_payments_manager.clone();
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_request_invoice(
                        db_clone,
                        lance_db_clone,
                        my_agent_payments_manager_clone,
                        bearer,
                        tool_key_name,
                        usage,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiPayInvoice {
                bearer,
                invoice_id,
                data_for_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                let my_agent_payments_manager_clone = self.my_agent_payments_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_pay_invoice(
                        db_clone,
                        lance_db_clone,
                        my_agent_payments_manager_clone,
                        bearer,
                        invoice_id,
                        data_for_tool,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListInvoices { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_invoices(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiAddCustomPrompt { bearer, prompt, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_custom_prompt(db_clone, lance_db_clone, bearer, prompt, res).await;
                });
            }
            NodeCommand::V2ApiDeleteCustomPrompt {
                bearer,
                prompt_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_delete_custom_prompt(db_clone, lance_db_clone, bearer, prompt_name, res).await;
                });
            }
            NodeCommand::V2ApiGetAllCustomPrompts { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_custom_prompts(db_clone, lance_db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetCustomPrompt {
                bearer,
                prompt_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_custom_prompt(db_clone, lance_db_clone, bearer, prompt_name, res).await;
                });
            }
            NodeCommand::V2ApiSearchCustomPrompts { bearer, query, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_search_custom_prompts(db_clone, lance_db_clone, bearer, query, res).await;
                });
            }
            NodeCommand::V2ApiUpdateCustomPrompt { bearer, prompt, res } => {
                let db_clone = Arc::clone(&self.db);
                let lance_db_clone = self.lance_db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_custom_prompt(db_clone, lance_db_clone, bearer, prompt, res).await;
                });
            }
            NodeCommand::V2ApiStopLLM {
                bearer,
                inbox_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let stopper_clone = self.llm_stopper.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_stop_llm(db_clone, stopper_clone, bearer, inbox_name, res).await;
                });
            }
            NodeCommand::V2ApiRetryMessage {
                bearer,
                inbox_name,
                message_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let job_manager_clone = self.job_manager.clone().unwrap();
                let node_encryption_sk_clone = self.encryption_secret_key.clone();
                let node_encryption_pk_clone = self.encryption_public_key.clone();
                let node_signing_sk_clone = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::v2_api_retry_message(
                        db_clone,
                        job_manager_clone,
                        node_encryption_sk_clone,
                        node_encryption_pk_clone,
                        node_signing_sk_clone,
                        bearer,
                        inbox_name,
                        message_id,
                        res,
                    )
                    .await;
                });
            }
            _ => (),
        }
    }
}
