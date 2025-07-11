use std::sync::Arc;

use shinkai_http_api::node_commands::NodeCommand;

use crate::{network::Node, utils::environment::fetch_node_environment};

impl Node {
    pub async fn handle_command(&self, command: NodeCommand) {
        match command {
            // Spawn a new task for each command to handle it concurrently

            // NodeCommand::Shutdown => {
            //     shinkai_log(ShinkaiLogOption::Node, ShinkaiLogLevel::Info, "Shutdown command received. Stopping the
            // node.");     // self.db = Arc::new(Mutex::new(ShinkaiDB::new("PLACEHOLDER").expect("Failed to
            // create a temporary database"))); },
            NodeCommand::PingAll => {
                let listen_address_clone = self.listen_address;
                let libp2p_manager_clone = self.libp2p_manager.clone();
                tokio::spawn(async move {
                    let _ = Self::ping_all(listen_address_clone, libp2p_manager_clone).await;
                });
            }
            NodeCommand::GetPublicKeys(sender) => {
                let identity_public_key = self.identity_public_key;
                let encryption_public_key = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::send_public_keys(identity_public_key, encryption_public_key, sender).await;
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
                let libp2p_event_sender = self.libp2p_event_sender.clone();
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
                        libp2p_event_sender,
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
            NodeCommand::V2ApiPublishAgent { bearer, agent_id, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let shinkai_name = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let signing_secret_key_clone = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_publish_agent(
                        db_clone,
                        bearer,
                        shinkai_name,
                        node_env,
                        agent_id,
                        identity_manager_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiExportAgent { bearer, agent_id, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let shinkai_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_export_agent(db_clone, bearer, shinkai_name, node_env, agent_id, res).await;
                });
            }
            NodeCommand::V2ApiImportAgent { bearer, url, res } => {
                let db_clone = Arc::clone(&self.db);
                let full_identity = self.node_name.clone();
                let signing_secret_key = self.identity_secret_key.clone();
                let node_env = fetch_node_environment();
                let embedding_generator = Arc::new(self.embedding_generator.clone());
                tokio::spawn(async move {
                    let _ = Node::v2_api_import_agent_url(
                        db_clone,
                        bearer,
                        full_identity,
                        url,
                        node_env,
                        signing_secret_key,
                        embedding_generator,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiImportAgentZip { bearer, file_data, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let full_identity = self.node_name.clone();
                let embedding_generator = Arc::new(self.embedding_generator.clone());
                tokio::spawn(async move {
                    let _ = Node::v2_api_import_agent_zip(
                        db_clone,
                        bearer,
                        full_identity,
                        node_env,
                        file_data,
                        embedding_generator,
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
                let public_https_certificate = self.public_https_certificate.clone();
                let libp2p_event_sender = self.libp2p_event_sender.clone();
                tokio::spawn(async move {
                    let _ = Node::api_handle_registration_code_usage(
                        db_clone,
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
                        public_https_certificate,
                        msg,
                        ws_manager_trait,
                        support_embedding_models,
                        libp2p_event_sender,
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
            // NodeCommand::APIGetAllInboxesForProfile { msg, res } => self.api_get_all_inboxes_for_profile(msg,
            // res).await,
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
            // NodeCommand::GetLastMessagesFromInboxWithBranches { inbox_name, limit, offset_key, res } =>
            // self.local_get_last_messages_from_inbox_with_branches(inbox_name, limit, offset_key, res).await,
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
            // NodeCommand::APIUpdateDefaultEmbeddingModel { msg, res } => self.api_update_default_embedding_model(msg,
            // res).await,
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
            NodeCommand::InternalCheckRustToolsInstallation { res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::internal_check_rust_tools_installation(db_clone, res).await;
                });
            }
            //
            // V2 API
            NodeCommand::V2ApiGetPublicKeys { res: sender } => {
                let identity_public_key = self.identity_public_key;
                let encryption_public_key = self.encryption_public_key;
                tokio::spawn(async move {
                    let _ = Node::v2_send_public_keys(identity_public_key, encryption_public_key, sender).await;
                });
            }
            NodeCommand::V2ApiInitialRegistration { payload, res } => {
                let db_clone = Arc::clone(&self.db);

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
                let public_https_certificate = self.public_https_certificate.clone();
                let libp2p_event_sender = self.libp2p_event_sender.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_handle_initial_registration(
                        db_clone,
                        identity_manager_clone,
                        node_name_clone,
                        payload,
                        public_https_certificate,
                        res,
                        first_device_needs_registration_code,
                        embedding_generator_clone,
                        job_manager,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        identity_secret_key_clone,
                        initial_llm_providers_clone,
                        ws_manager_trait,
                        supported_embedding_models,
                        libp2p_event_sender,
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
                        None,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAddMessagesGodMode {
                bearer,
                job_id,
                messages,
                res,
            } => {
                let db_clone = self.db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let signing_secret_key_clone = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_add_messages_god_mode(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        bearer,
                        job_id,
                        messages,
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
            NodeCommand::V2ApiGetAllSmartInboxes {
                bearer,
                limit,
                offset,
                show_hidden,
                agent_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_get_all_smart_inboxes(
                        db_clone,
                        identity_manager_clone,
                        bearer,
                        limit,
                        offset,
                        show_hidden,
                        agent_id,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetAllSmartInboxesPaginated {
                bearer,
                limit,
                offset,
                show_hidden,
                agent_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_get_all_smart_inboxes_paginated(
                        db_clone,
                        identity_manager_clone,
                        bearer,
                        limit,
                        offset,
                        show_hidden,
                        agent_id,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAvailableLLMProviders { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_get_available_llm_providers(db_clone, node_name_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiForkJobMessages {
                bearer,
                job_id,
                message_id,
                res,
            } => {
                let db_clone = self.db.clone();
                let node_name_clone = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let signing_secret_key_clone = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_fork_job_messages(
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        bearer,
                        job_id,
                        message_id,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiRemoveJob { bearer, job_id, res } => {
                let db_clone = self.db.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_remove_job(db_clone, bearer, job_id, res).await;
                });
            }
            NodeCommand::V2ApiVecFSRetrievePathSimplifiedJson { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_vec_fs_retrieve_path_simplified_json(
                        db_clone,
                        identity_manager_clone,
                        payload,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiVecFSCreateFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_create_folder(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }
            NodeCommand::V2ApiMoveItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_move_item(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiCopyItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_copy_item(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiMoveFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_move_folder(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiCopyFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_copy_folder(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiDeleteFolder { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_delete_folder(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiDeleteItem { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_delete_item(db_clone, identity_manager_clone, payload, bearer, res).await;
                });
            }

            NodeCommand::V2ApiSearchItems { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                let embedding_generator_clone = self.embedding_generator.clone();

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_search_items(
                        db_clone,
                        identity_manager_clone,
                        payload,
                        Arc::new(embedding_generator_clone),
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiSearchFilesByName { bearer, name, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_search_files_by_name(db_clone, identity_manager_clone, name, bearer, res).await;
                });
            }
            NodeCommand::V2ApiVecFSRetrieveVectorResource { bearer, path, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_retrieve_vector_resource(db_clone, identity_manager_clone, path, bearer, res).await;
                });
            }
            NodeCommand::V2ApiVecFSRetrieveFilesForJob { bearer, job_id, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_vec_fs_retrieve_files_for_job(
                        db_clone,
                        identity_manager_clone,
                        job_id,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiVecFSGetFolderNameForJob { bearer, job_id, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_vec_fs_get_folder_name_for_job(
                        db_clone,
                        identity_manager_clone,
                        job_id,
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
            NodeCommand::V2ApiUploadFileToFolder {
                bearer,
                filename,
                file,
                path,
                file_datetime,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                let embedding_generator_clone = self.embedding_generator.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_upload_file_to_folder(
                        db_clone,
                        identity_manager_clone,
                        Arc::new(embedding_generator_clone),
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
            NodeCommand::V2ApiUploadFileToJob {
                bearer,
                job_id,
                filename,
                file,
                file_datetime,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let embedding_generator_clone = self.embedding_generator.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_upload_file_to_job(
                        db_clone,
                        identity_manager_clone,
                        Arc::new(embedding_generator_clone),
                        bearer,
                        job_id,
                        filename,
                        file,
                        file_datetime,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiRetrieveFile { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);

                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_retrieve_file(db_clone, identity_manager_clone, payload, bearer, res).await;
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
            NodeCommand::V2ApiTestLlmProvider { bearer, provider, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone();
                let identity_secret_key_clone = self.identity_secret_key.clone();
                let ws_manager_trait = self.ws_manager_trait.clone();
                let node_encryption_sk_clone = self.encryption_secret_key.clone();
                let node_encryption_pk_clone = self.encryption_public_key.clone();
                let node_signing_sk_clone = self.identity_secret_key.clone();
                let node_name_clone = self.node_name.clone();

                tokio::spawn(async move {
                    let _ = Node::v2_api_test_llm_provider(
                        db_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        identity_secret_key_clone,
                        bearer,
                        provider,
                        node_name_clone,
                        node_encryption_sk_clone,
                        node_encryption_pk_clone,
                        node_signing_sk_clone,
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
            NodeCommand::V2ApiGetJobProvider { bearer, job_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_job_provider(db_clone, bearer, job_id, res).await;
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
            NodeCommand::V2ApiShinkaiBackendGetQuota {
                bearer,
                model_type,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_check_shinkai_backend_quota(db_clone, model_type, bearer, res).await;
                });
            }
            NodeCommand::V2ApiIsPristine { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_is_pristine(bearer, db_clone, res).await;
                });
            }
            NodeCommand::V2ApiHealthCheck { res } => {
                let db_clone = Arc::clone(&self.db);
                let public_https_certificate_clone = self.public_https_certificate.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_health_check(db_clone, public_https_certificate_clone, res).await;
                });
            }
            NodeCommand::V2ApiScanOllamaModels { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_scan_ollama_models(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiListAllShinkaiTools { bearer, category, res } => {
                let db_clone = Arc::clone(&self.db);
                let tool_router_clone = self.tool_router.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_shinkai_tools(
                        db_clone,
                        bearer,
                        node_name_clone,
                        category,
                        tool_router_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListAllMcpShinkaiTools { category, res } => {
                let db_clone = Arc::clone(&self.db);
                let tool_router_clone = self.tool_router.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_mcp_shinkai_tools(
                        db_clone,
                        node_name_clone,
                        category,
                        tool_router_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListAllNetworkShinkaiTools { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let tool_router_clone = self.tool_router.clone();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_network_shinkai_tools(
                        db_clone,
                        bearer,
                        node_name_clone,
                        tool_router_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListAllShinkaiToolsVersions { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_shinkai_tools_versions(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetShinkaiToolMetadata {
                bearer,
                tool_router_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_shinkai_tool_metadata(db_clone, bearer, tool_router_key, res).await;
                });
            }
            NodeCommand::V2ApiGetToolWithOffering {
                bearer,
                tool_key_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_tool_with_offering(db_clone, node_name_clone, bearer, tool_key_name, res)
                        .await;
                });
            }
            NodeCommand::V2ApiGetToolsWithOfferings { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_tools_with_offerings(db_clone, node_name_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiSetShinkaiTool {
                bearer,
                tool_key,
                payload,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_shinkai_tool(db_clone, bearer, tool_key, payload, res).await;
                });
            }
            NodeCommand::V2ApiAddShinkaiTool {
                bearer,
                shinkai_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_shinkai_tool(db_clone, bearer, node_env, shinkai_tool, res).await;
                });
            }
            NodeCommand::V2ApiAddNetworkAgent {
                bearer,
                shinkai_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_network_agent(
                        db_clone,
                        bearer,
                        node_env,
                        shinkai_tool,
                        identity_manager_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetShinkaiTool {
                bearer,
                payload,
                serialize_config,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_shinkai_tool(db_clone, bearer, payload, serialize_config, res).await;
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
            // TODO: repurpose
            // NodeCommand::V2ApiDownloadFileFromInbox {
            //     bearer,
            //     inbox_name,
            //     filename,
            //     res,
            // } => {
            //     let db_clone = Arc::clone(&self.db);
            //     tokio::spawn(async move {
            //         let _ = Node::v2_api_download_file_from_inbox(db_clone, bearer, inbox_name, filename, res).await;
            //     });
            // }
            // NodeCommand::V2ApiListFilesInInbox {
            //     bearer,
            //     inbox_name,
            //     res,
            // } => {
            //     let db_clone = Arc::clone(&self.db);
            //     tokio::spawn(async move {
            //         let _ = Node::v2_api_list_files_in_inbox(db_clone, bearer, inbox_name, res).await;
            //     });
            // }
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
            NodeCommand::V2ApiGetAgentNetworkOffering {
                bearer,
                node_name,
                auto_check,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let my_agent_payments_manager_clone = self.my_agent_payments_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_agent_network_offering(
                        db_clone,
                        my_agent_payments_manager_clone,
                        bearer,
                        node_name,
                        auto_check,
                        res,
                    )
                    .await;
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
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_tool_offering(db_clone, bearer, res).await;
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
            NodeCommand::V2ApiSetToolOffering {
                bearer,
                tool_offering,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_tool_offering(db_clone, bearer, tool_offering, res).await;
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
                let wallet_manager_clone = self.wallet_manager.clone();
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_restore_coinbase_mpc_wallet(
                        db_clone,
                        wallet_manager_clone,
                        bearer,
                        network,
                        config,
                        wallet_id,
                        role,
                        node_name,
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
                let wallet_manager_clone = self.wallet_manager.clone();
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_create_coinbase_mpc_wallet(
                        db_clone,
                        wallet_manager_clone,
                        bearer,
                        network,
                        config,
                        role,
                        node_name,
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
            NodeCommand::V2ApiGetWalletBalance { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let wallet_manager_clone = self.wallet_manager.clone();
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_get_wallet_balance(db_clone, wallet_manager_clone, bearer, node_name, res).await;
                });
            }
            NodeCommand::V2ApiGetStorageLocation { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_storage_location(db_clone, bearer, res).await;
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
                tokio::spawn(async move {
                    let _ = Node::v2_api_request_invoice(
                        db_clone,
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
                let my_agent_payments_manager_clone = self.my_agent_payments_manager.clone();
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_pay_invoice(
                        db_clone,
                        my_agent_payments_manager_clone,
                        bearer,
                        invoice_id,
                        data_for_tool,
                        node_name,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiRejectInvoice {
                bearer,
                invoice_id,
                reason,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let my_agent_payments_manager_clone = self.my_agent_payments_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_reject_invoice(
                        db_clone,
                        my_agent_payments_manager_clone,
                        bearer,
                        invoice_id,
                        reason,
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
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_custom_prompt(db_clone, bearer, prompt, res).await;
                });
            }
            NodeCommand::V2ApiDeleteCustomPrompt {
                bearer,
                prompt_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_delete_custom_prompt(db_clone, bearer, prompt_name, res).await;
                });
            }
            NodeCommand::V2ApiGetAllCustomPrompts { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_custom_prompts(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetCustomPrompt {
                bearer,
                prompt_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_custom_prompt(db_clone, bearer, prompt_name, res).await;
                });
            }
            NodeCommand::V2ApiSearchCustomPrompts { bearer, query, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_search_custom_prompts(db_clone, bearer, query, res).await;
                });
            }
            NodeCommand::V2ApiUpdateCustomPrompt { bearer, prompt, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_custom_prompt(db_clone, bearer, prompt, res).await;
                });
            }
            NodeCommand::V2ApiStopLLM {
                bearer,
                inbox_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let stopper_clone = self.llm_stopper.clone();
                let job_manager_clone = self.job_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_stop_llm(db_clone, stopper_clone, bearer, inbox_name, job_manager_clone, res)
                        .await;
                });
            }
            NodeCommand::V2ApiAddAgent { bearer, agent, res } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_agent(db_clone, identity_manager_clone, bearer, agent, res).await;
                });
            }
            NodeCommand::V2ApiRemoveAgent { bearer, agent_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_agent(db_clone, bearer, agent_id, res).await;
                });
            }
            NodeCommand::V2ApiUpdateAgent {
                bearer,
                partial_agent,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let full_identity = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_agent(db_clone, bearer, full_identity, partial_agent, res).await;
                });
            }
            NodeCommand::V2ApiGetAgent { bearer, agent_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_agent(db_clone, bearer, agent_id, res).await;
                });
            }
            NodeCommand::V2ApiGetAllAgents { bearer, filter, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_agents(db_clone, bearer, filter, res).await;
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
            NodeCommand::V2ApiUpdateJobScope {
                bearer,
                job_id,
                job_scope,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_job_scope(db_clone, bearer, job_id, job_scope, res).await;
                });
            }
            NodeCommand::V2ApiGetJobScope { bearer, job_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_job_scope(db_clone, bearer, job_id, res).await;
                });
            }
            NodeCommand::V2ApiGetMessageTraces {
                bearer,
                message_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_message_traces(db_clone, bearer, message_id, res).await;
                });
            }
            // NodeCommand::V2ApiGetToolingLogs {
            //     bearer,
            //     message_id,
            //     res,
            // } => {
            //     let db_clone = Arc::clone(&self.db);
            //     let sqlite_logger_clone = Arc::clone(&self.sqlite_logger);
            //     tokio::spawn(async move {
            //         let _ = Node::v2_api_get_tooling_logs(db_clone, sqlite_logger_clone, bearer, message_id,
            // res).await;     });
            // }
            NodeCommand::V2ApiExecuteTool {
                bearer,
                tool_router_key,
                parameters,
                tool_id,
                app_id,
                agent_id,
                llm_provider,
                extra_config,
                mounts,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);

                let node_name = self.node_name.clone();
                let job_manager = self.job_manager.clone().unwrap();
                let identity_manager = self.identity_manager.clone();
                let encryption_secret_key = self.encryption_secret_key.clone();
                let encryption_public_key = self.encryption_public_key;
                let signing_secret_key = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::execute_tool(
                        bearer,
                        node_name,
                        db_clone,
                        tool_router_key,
                        parameters,
                        tool_id,
                        app_id,
                        agent_id,
                        llm_provider,
                        extra_config,
                        identity_manager,
                        job_manager,
                        encryption_secret_key,
                        encryption_public_key,
                        signing_secret_key,
                        mounts,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiExecuteMcpTool {
                tool_router_key,
                parameters,
                tool_id,
                app_id,
                agent_id,
                extra_config,
                mounts,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name = self.node_name.clone();
                let job_manager = self.job_manager.clone().unwrap();
                let identity_manager = self.identity_manager.clone();
                let encryption_secret_key = self.encryption_secret_key.clone();
                let encryption_public_key = self.encryption_public_key;
                let signing_secret_key = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::execute_mcp_tool(
                        node_name,
                        db_clone,
                        tool_router_key,
                        parameters,
                        tool_id,
                        app_id,
                        agent_id,
                        extra_config,
                        identity_manager,
                        job_manager,
                        encryption_secret_key,
                        encryption_public_key,
                        signing_secret_key,
                        mounts,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiExecuteCode {
                bearer,
                code,
                tools,
                tool_type,
                parameters,
                extra_config,
                oauth,
                tool_id,
                app_id,
                agent_id,
                llm_provider,
                mounts,
                runner,
                operating_system,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name = self.node_name.clone();
                let identity_manager_clone = self.identity_manager.clone();
                let job_manager_clone = self.job_manager.clone().unwrap();
                let encryption_secret_key_clone = self.encryption_secret_key.clone();
                let encryption_public_key_clone = self.encryption_public_key;
                let signing_secret_key_clone = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::run_execute_code(
                        bearer,
                        db_clone,
                        tool_type,
                        code,
                        tools,
                        parameters,
                        extra_config,
                        oauth,
                        tool_id,
                        app_id,
                        agent_id,
                        llm_provider,
                        node_name,
                        mounts,
                        runner,
                        operating_system,
                        identity_manager_clone,
                        job_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGenerateToolDefinitions {
                bearer,
                language,
                tools,
                res,
            } => {
                let db_clone = self.db.clone();

                tokio::spawn(async move {
                    let _ = Node::get_tool_definitions(bearer, db_clone, language, tools, res).await;
                });
            }
            NodeCommand::V2ApiGenerateToolFetchQuery {
                bearer,
                language,
                tools,
                code,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();

                tokio::spawn(async move {
                    let _ = Node::generate_tool_fetch_query(
                        bearer,
                        db_clone,
                        language,
                        tools,
                        code,
                        identity_manager_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGenerateToolImplementation {
                bearer,
                message,
                language,
                tools,
                post_check,
                raw,
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
                    let _ = Node::generate_tool_implementation(
                        bearer,
                        db_clone,
                        message,
                        language,
                        tools,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        post_check,
                        raw,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiToolImplementationUndoTo {
                bearer,
                message_hash,
                job_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_tool_implementation_undo_to(bearer, db_clone, message_hash, job_id, res).await;
                });
            }
            NodeCommand::V2ApiToolImplementationCodeUpdate {
                bearer,
                job_id,
                code,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let identity_manager_clone = self.identity_manager.clone();
                let node_name_clone = self.node_name.clone();
                let node_encryption_sk_clone = self.encryption_secret_key.clone();
                let node_encryption_pk_clone = self.encryption_public_key.clone();
                let node_signing_sk_clone = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::v2_api_tool_implementation_code_update(
                        bearer,
                        db_clone,
                        job_id,
                        code,
                        identity_manager_clone,
                        node_name_clone,
                        node_encryption_sk_clone,
                        node_encryption_pk_clone,
                        node_signing_sk_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiExportTool {
                bearer,
                tool_key_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let shinkai_name = self.node_name.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_export_tool(db_clone, bearer, shinkai_name, node_env, tool_key_path, res).await;
                });
            }
            NodeCommand::V2ApiPublishTool {
                bearer,
                tool_key_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let shinkai_name = self.node_name.clone();
                let identity_manager = self.identity_manager.clone();
                let signing_secret_key = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_publish_tool(
                        db_clone,
                        bearer,
                        shinkai_name,
                        node_env,
                        tool_key_path,
                        identity_manager,
                        signing_secret_key,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiImportTool { bearer, url, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let signing_secret_key = self.identity_secret_key.clone();
                let embedding_generator = self.embedding_generator.clone();
                let full_identity = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_import_tool_url(
                        db_clone,
                        bearer,
                        full_identity,
                        node_env,
                        url,
                        signing_secret_key,
                        Arc::new(embedding_generator),
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiImportToolZip { bearer, file_data, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let embedding_generator = self.embedding_generator.clone();
                let full_identity = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_import_tool_zip(
                        db_clone,
                        bearer,
                        full_identity,
                        node_env,
                        file_data,
                        Arc::new(embedding_generator),
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiRemoveTool { bearer, tool_key, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_tool(db_clone, bearer, tool_key, res).await;
                });
            }
            NodeCommand::V2ApiResolveShinkaiFileProtocol {
                bearer,
                shinkai_file_protocol,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                // Get the node storage path
                let node_env = fetch_node_environment();
                let node_storage_path = node_env.node_storage_path.unwrap_or_default();
                tokio::spawn(async move {
                    let _ = Node::v2_api_resolve_shinkai_file_protocol(
                        bearer,
                        db_clone,
                        shinkai_file_protocol,
                        node_storage_path,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAddCronTask {
                bearer,
                cron,
                action,
                name,
                description,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_cron_task(db_clone, bearer, cron, action, name, description, res).await;
                });
            }
            NodeCommand::V2ApiUpdateCronTask {
                bearer,
                cron_task_id,
                cron,
                action,
                name,
                description,
                paused,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_cron_task(
                        db_clone,
                        bearer,
                        cron_task_id,
                        cron,
                        action,
                        name,
                        description,
                        paused,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiForceExecuteCronTask {
                bearer,
                cron_task_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let cron_manager_clone = self.cron_manager.clone().unwrap();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_force_execute_cron_task(db_clone, cron_manager_clone, bearer, cron_task_id, res)
                            .await;
                });
            }
            NodeCommand::V2ApiGetCronSchedule { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let cron_manager_clone = self.cron_manager.clone().unwrap();
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_cron_schedule(db_clone, cron_manager_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiListAllCronTasks { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_all_cron_tasks(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetSpecificCronTask {
                bearer,
                cron_task_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_specific_cron_task(db_clone, bearer, cron_task_id, res).await;
                });
            }
            NodeCommand::V2ApiRemoveCronTask {
                bearer,
                cron_task_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_cron_task(db_clone, bearer, cron_task_id, res).await;
                });
            }
            NodeCommand::V2ApiGetCronTaskLogs {
                bearer,
                cron_task_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_cron_task_logs(db_clone, bearer, cron_task_id, res).await;
                });
            }
            NodeCommand::V2ApiImportCronTask { bearer, url, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name = self.node_name.node_name.clone();
                let signing_secret_key = self.identity_secret_key.clone();
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_import_cron_task(db_clone, bearer, url, node_name, signing_secret_key, res).await;
                });
            }
            NodeCommand::V2ApiExportCronTask {
                bearer,
                cron_task_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_export_cron_task(db_clone, bearer, cron_task_id, res).await;
                });
            }
            NodeCommand::V2ApiGenerateToolMetadataImplementation {
                bearer,
                job_id,
                language,
                tools,
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
                    let _ = Node::generate_tool_metadata_implementation(
                        bearer,
                        job_id,
                        language,
                        tools,
                        db_clone,
                        node_name_clone,
                        identity_manager_clone,
                        job_manager_clone,
                        encryption_secret_key_clone,
                        encryption_public_key_clone,
                        signing_secret_key_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiExportMessagesFromInbox {
                bearer,
                inbox_name,
                format,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_export_messages_from_inbox(db_clone, bearer, inbox_name, format, res).await;
                });
            }
            NodeCommand::V2ApiSearchShinkaiTool {
                bearer,
                query,
                agent_or_llm,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_search_shinkai_tool(db_clone, bearer, query, agent_or_llm, res).await;
                });
            }
            NodeCommand::V2ApiSetPlaygroundTool {
                bearer,
                payload,
                tool_id,
                app_id,
                original_tool_key_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_playground_tool(
                        db_clone,
                        bearer,
                        payload,
                        node_env,
                        tool_id,
                        app_id,
                        original_tool_key_path,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListPlaygroundTools { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_playground_tools(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiRemovePlaygroundTool { bearer, tool_key, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_remove_playground_tool(db_clone, bearer, tool_key, res).await;
                });
            }
            NodeCommand::V2ApiGetPlaygroundTool { bearer, tool_key, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_playground_tool(db_clone, bearer, tool_key, res).await;
                });
            }
            NodeCommand::V2ApiGetOAuthToken {
                bearer,
                connection_name,
                tool_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_oauth_token(db_clone, bearer, connection_name, tool_key, res).await;
                });
            }
            NodeCommand::V2ApiSetOAuthToken {
                bearer,
                code,
                state,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_oauth_token(db_clone, bearer, code, state, res).await;
                });
            }
            NodeCommand::V2ApiUploadToolAsset {
                bearer,
                tool_id,
                app_id,
                file_name,
                file_data,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_upload_tool_asset(
                        db_clone, bearer, tool_id, app_id, file_name, file_data, node_env, res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiUploadPlaygroundFile {
                bearer,
                tool_id,
                app_id,
                file_name,
                file_data,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_upload_playground_file(
                        db_clone, bearer, tool_id, app_id, file_name, file_data, node_env, res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiListToolAssets {
                bearer,
                tool_id,
                app_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_tool_assets(db_clone, bearer, tool_id, app_id, node_env, res).await;
                });
            }
            NodeCommand::V2ApiDeleteToolAsset {
                bearer,
                tool_id,
                app_id,
                file_name,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_delete_tool_asset(db_clone, bearer, tool_id, app_id, file_name, node_env, res)
                        .await;
                });
            }
            NodeCommand::V2ApiEnableAllTools { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_enable_all_tools(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiDisableAllTools { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_disable_all_tools(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiDuplicateTool {
                bearer,
                tool_key_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let identity_manager = self.identity_manager.clone();
                let job_manager = self.job_manager.clone();
                let encryption_secret_key = self.encryption_secret_key.clone();
                let encryption_public_key = self.encryption_public_key.clone();
                let signing_secret_key = self.identity_secret_key.clone();

                tokio::spawn(async move {
                    let _ = Node::v2_api_duplicate_tool(
                        db_clone,
                        bearer,
                        tool_key_path,
                        node_name_clone,
                        identity_manager,
                        job_manager,
                        encryption_secret_key,
                        encryption_public_key,
                        signing_secret_key,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiAddRegexPattern {
                bearer,
                provider_name,
                pattern,
                response,
                description,
                priority,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_regex_pattern(
                        db_clone,
                        bearer,
                        provider_name,
                        pattern,
                        response,
                        description,
                        priority,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiStoreProxy {
                bearer,
                tool_router_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_store_proxy(db_clone, bearer, tool_router_key, res).await;
                });
            }
            NodeCommand::V2ApiStandAlonePlayground {
                bearer,
                code,
                metadata,
                assets,
                language,
                tools,
                parameters,
                config,
                oauth,
                tool_id,
                app_id,
                llm_provider,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_standalone_playground(
                        db_clone,
                        node_name_clone,
                        bearer,
                        node_env,
                        code,
                        metadata,
                        assets,
                        language,
                        tools,
                        parameters,
                        config,
                        oauth,
                        tool_id,
                        app_id,
                        llm_provider,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiCheckDefaultToolsSync { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_check_default_tools_sync(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiComputeQuestsStatus { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let encryption_public_key_clone = self.encryption_public_key.clone();
                let identity_public_key_clone = self.identity_public_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_compute_quests_status(
                        db_clone,
                        node_name_clone,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        bearer,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiComputeAndSendQuestsStatus { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                let encryption_public_key_clone = self.encryption_public_key.clone();
                let identity_public_key_clone = self.identity_public_key.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_compute_and_send_quests_status(
                        db_clone,
                        node_name_clone,
                        encryption_public_key_clone,
                        identity_public_key_clone,
                        bearer,
                        res,
                    )
                    .await;
                });
            }

            NodeCommand::V2ApiListMCPServers { bearer, res } => {
                let db_clone: Arc<shinkai_sqlite::SqliteManager> = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_list_mcp_servers(db_clone, bearer, res).await;
                });
            }

            NodeCommand::V2ApiAddMCPServer {
                bearer,
                mcp_server,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_mcp_server(db_clone, node_name_clone, bearer, mcp_server, res).await;
                });
            }

            NodeCommand::V2ApiUpdateMCPServer {
                bearer,
                mcp_server,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_name_clone = self.node_name.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_update_mcp_server(db_clone, bearer, mcp_server, &node_name_clone, res).await;
                });
            }

            NodeCommand::V2ApiImportMCPServerFromGitHubURL {
                bearer,
                github_url,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_import_mcp_server_from_github_url(db_clone, bearer, github_url, res).await;
                });
            }

            NodeCommand::V2ApiDeleteMCPServer {
                bearer,
                mcp_server_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_delete_mcp_server(db_clone, bearer, mcp_server_id, res).await;
                });
            }

            NodeCommand::V2ApiSetEnableMCPServer {
                bearer,
                mcp_server_id,
                is_enabled,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_enable_mcp_server(db_clone, bearer, mcp_server_id, is_enabled, res).await;
                });
            }

            NodeCommand::V2ApiGetAllMCPServerTools {
                bearer,
                mcp_server_id,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_all_mcp_server_tools(db_clone, bearer, mcp_server_id, res).await;
                });
            }

            NodeCommand::V2ApiSetToolEnabled {
                bearer,
                tool_router_key,
                enabled,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_tool_enabled(db_clone, bearer, tool_router_key, enabled, res).await;
                });
            }
            NodeCommand::V2ApiSetToolMcpEnabled {
                bearer,
                tool_router_key,
                mcp_enabled,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ =
                        Node::v2_api_set_tool_mcp_enabled(db_clone, bearer, tool_router_key, mcp_enabled, res).await;
                });
            }
            NodeCommand::V2ApiCopyToolAssets {
                bearer,
                is_first_playground,
                first_path,
                is_second_playground,
                second_path,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_copy_tool_assets(
                        db_clone,
                        bearer,
                        node_env,
                        is_first_playground,
                        first_path,
                        is_second_playground,
                        second_path,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetToolsFromToolset {
                bearer,
                tool_set_key,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_tools_from_toolset(db_clone, bearer, tool_set_key, res).await;
                });
            }
            NodeCommand::V2SetCommonToolSetConfig {
                bearer,
                tool_set_key,
                value,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_common_toolset_config(db_clone, bearer, tool_set_key, value, res).await;
                });
            }
            NodeCommand::V2ApiCheckTool {
                bearer,
                code,
                language,
                additional_headers,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);

                tokio::spawn(async move {
                    let _ = Node::check_tool(bearer, db_clone, code, language, additional_headers, res).await;
                });
            }
            NodeCommand::V2ApiSetPreferences { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_preferences(db_clone, bearer, payload, res).await;
                });
            }
            NodeCommand::V2ApiGetPreferences { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_preferences(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiGetLastUsedAgentsAndLLMs { bearer, last, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_last_used_agents_and_llms(db_clone, bearer, last, res).await;
                });
            }
            NodeCommand::V2ApiDockerStatus { res } => {
                tokio::spawn(async move {
                    let _ = Node::v2_api_docker_status(res).await;
                });
            }
            NodeCommand::V2ApiSetNgrokAuthToken {
                bearer,
                auth_token,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_ngrok_auth_token(db_clone, bearer, auth_token, res).await;
                });
            }
            NodeCommand::V2ApiClearNgrokAuthToken { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_clear_ngrok_auth_token(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiSetNgrokEnabled { bearer, enabled, res } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_set_ngrok_enabled(db_clone, bearer, enabled, node_env.api_listen_address, res)
                        .await;
                });
            }
            NodeCommand::V2ApiGetNgrokStatus { bearer, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_ngrok_status(db_clone, bearer, res).await;
                });
            }
            NodeCommand::V2ApiAddShinkaiTool {
                bearer,
                shinkai_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_shinkai_tool(db_clone, bearer, node_env, shinkai_tool, res).await;
                });
            }
            NodeCommand::V2ApiAddNetworkAgent {
                bearer,
                shinkai_tool,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                let node_env = fetch_node_environment();
                let identity_manager_clone = self.identity_manager.clone();
                tokio::spawn(async move {
                    let _ = Node::v2_api_add_network_agent(
                        db_clone,
                        bearer,
                        node_env,
                        shinkai_tool,
                        identity_manager_clone,
                        res,
                    )
                    .await;
                });
            }
            NodeCommand::V2ApiGetShinkaiTool {
                bearer,
                payload,
                serialize_config,
                res,
            } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_shinkai_tool(db_clone, bearer, payload, serialize_config, res).await;
                });
            }
            _ => (),
        }
    }
}
