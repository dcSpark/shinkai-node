use crate::managers::identity_manager::IdentityManagerTrait;
use crate::{
    llm_provider::job_manager::JobManager, managers::IdentityManager, network::{
        node::ProxyConnectionInfo, node_error::NodeError, node_shareable_logic::validate_message_main_logic, Node
    }
};
use async_channel::Sender;
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::error;
use reqwest::StatusCode;

use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_embedding::model_type::EmbeddingModelType;
use shinkai_http_api::node_api_router::{APIError, APIUseRegistrationCodeSuccessResponse, SendResponseBodyData};
use shinkai_message_primitives::schemas::identity::{
    DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity, StandardIdentityType
};
use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName, llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::{ShinkaiName, ShinkaiSubidentityType}
    }, shinkai_message::{
        shinkai_message::ShinkaiMessage, shinkai_message_schemas::{IdentityPermissions, MessageSchemaType, RegistrationCodeType}
    }, shinkai_utils::{
        encryption::{
            clone_static_secret_key, encryption_public_key_to_string, string_to_encryption_public_key, EncryptionMethod
        }, shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}, signatures::{clone_signature_secret_key, signature_public_key_to_string, string_to_signature_public_key}
    }
};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;

use std::{env, sync::Arc};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn validate_message(
        encryption_secret_key: EncryptionStaticKey,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: &ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: Option<MessageSchemaType>,
    ) -> Result<(ShinkaiMessage, Identity), APIError> {
        validate_message_main_logic(
            &encryption_secret_key,
            identity_manager,
            &node_name.clone(),
            potentially_encrypted_msg,
            schema_type,
        )
        .await
    }

    async fn has_standard_identity_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        std_identity: &StandardIdentity,
    ) -> Result<bool, NodeError> {
        let has_permission = db
            .has_permission(&inbox_name.to_string(), std_identity, InboxPermission::Read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(has_permission)
    }

    async fn has_device_identity_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        std_identity: &DeviceIdentity,
    ) -> Result<bool, NodeError> {
        let std_device = std_identity.clone().to_standard_identity().ok_or(NodeError {
            message: "Failed to convert to standard identity".to_string(),
        })?;
        Self::has_standard_identity_access(db, inbox_name, &std_device).await
    }

    pub async fn has_inbox_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        sender_subidentity: &Identity,
    ) -> Result<bool, NodeError> {
        let sender_shinkai_name = ShinkaiName::new(sender_subidentity.get_full_identity_name())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let has_creation_permission = inbox_name.has_creation_access(sender_shinkai_name);
        if let Ok(true) = has_creation_permission {
            println!("has_creation_permission: true");
            return Ok(true);
        }

        match sender_subidentity {
            Identity::Standard(std_identity) => Self::has_standard_identity_access(db, inbox_name, std_identity).await,
            Identity::Device(std_device) => Self::has_device_identity_access(db, inbox_name, std_device).await,
            _ => Err(NodeError {
                message: format!(
                    "Invalid Identity type. You don't have enough permissions to access the inbox: {}",
                    inbox_name
                ),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_registration_code_usage(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        first_device_needs_registration_code: bool,
        _embedding_generator: Arc<RemoteEmbeddingGenerator>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        identity_secret_key: SigningKey,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        registration_code: RegistrationCode,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
        public_https_certificate: Option<String>,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("handle_registration_code_usage");

        let mut code = registration_code.code;
        let registration_name = registration_code.registration_name;
        let profile_identity_pk = registration_code.profile_identity_pk;
        let profile_encryption_pk = registration_code.profile_encryption_pk;
        let device_identity_pk = registration_code.device_identity_pk;
        let device_encryption_pk = registration_code.device_encryption_pk;
        let identity_type = registration_code.identity_type;
        // Comment (to me): this should be able to handle Device and Agent identities
        // why are we forcing standard_idendity_type?
        // let standard_identity_type = identity_type.to_standard().unwrap();
        let permission_type = registration_code.permission_type;

        // if first_device_registration_needs_code is false
        // then create a new registration code and use it
        // else use the code provided
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!(
                "registration code usage> first device needs registration code?: {:?}",
                first_device_needs_registration_code
            )
            .as_str(),
        );

        let main_profile_exists = match db.main_profile_exists(node_name.get_node_name_string().as_str()) {
            Ok(exists) => exists,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to check if main profile exists: {}", err),
                    }))
                    .await;
                return Ok(());
            }
        };

        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Debug,
            format!(
                "registration code usage> main_profile_exists: {:?}",
                main_profile_exists
            )
            .as_str(),
        );

        if !first_device_needs_registration_code && !main_profile_exists {
            let code_type = RegistrationCodeType::Device("main".to_string());
            let permissions = IdentityPermissions::Admin;

            match db.generate_registration_new_code(permissions, code_type) {
                Ok(new_code) => {
                    code = new_code;
                }
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to generate registration code: {}", err),
                        }))
                        .await;
                }
            }
        }

        let result = db
            .use_registration_code(
                &code.clone(),
                node_name.get_node_name_string().as_str(),
                registration_name.as_str(),
                &profile_identity_pk,
                &profile_encryption_pk,
                Some(&device_identity_pk),
                Some(&device_encryption_pk),
            )
            .map_err(|e| e.to_string())
            .map(|_| "true".to_string());

        match result {
            Ok(success) => {
                match identity_type {
                    IdentityType::Profile | IdentityType::Global => {
                        // Existing logic for handling profile identity
                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(),
                        // profile_name.clone());

                        let full_identity_name_result = ShinkaiName::from_node_and_profile_names(
                            node_name.get_node_name_string(),
                            registration_name.clone(),
                        );

                        if let Err(err) = &full_identity_name_result {
                            error!("Failed to add subidentity: {}", err);
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to add device subidentity: {}", err),
                                }))
                                .await;
                        }

                        let full_identity_name = full_identity_name_result.unwrap();
                        let standard_identity_type = identity_type.to_standard().unwrap();

                        let subidentity = StandardIdentity {
                            full_identity_name,
                            addr: None,
                            profile_signature_public_key: Some(signature_pk_obj),
                            profile_encryption_public_key: Some(encryption_pk_obj),
                            node_encryption_public_key: encryption_public_key,
                            node_signature_public_key: identity_public_key,
                            identity_type: standard_identity_type,
                            permission_type,
                        };

                        let api_v2_key = match db.read_api_v2_key() {
                            Ok(Some(api_key)) => api_key,
                            Ok(None) | Err(_) => {
                                let api_error = APIError {
                                    code: StatusCode::UNAUTHORIZED.as_u16(),
                                    error: "Unauthorized".to_string(),
                                    message: "Invalid bearer token".to_string(),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        };

                        let mut subidentity_manager = identity_manager.lock().await;
                        match subidentity_manager.add_profile_subidentity(subidentity).await {
                            Ok(_) => {
                                std::mem::drop(subidentity_manager);
                                if !first_device_needs_registration_code && !main_profile_exists {
                                    // Call the new function to scan and add Ollama models
                                    if let Err(err) = Self::scan_and_add_ollama_models(
                                        db.clone(),
                                        identity_manager.clone(),
                                        job_manager.clone(),
                                        identity_secret_key.clone(),
                                        node_name.clone(),
                                        ws_manager.clone(),
                                    )
                                    .await
                                    {
                                        error!("Failed to scan and add Ollama models: {}", err);
                                        // Note: We're not failing the entire operation if this fails
                                    }
                                }

                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: node_name.get_node_name_string().clone(),
                                    encryption_public_key: encryption_public_key_to_string(encryption_public_key),
                                    identity_public_key: signature_public_key_to_string(identity_public_key),
                                    api_v2_key,
                                    api_v2_cert: public_https_certificate,
                                };
                                let _ = res.send(Ok(success_response)).await.map_err(|_| ());
                            }
                            Err(err) => {
                                error!("Failed to add subidentity: {}", err);
                                let _ = res
                                    .send(Err(APIError {
                                        code: StatusCode::BAD_REQUEST.as_u16(),
                                        error: "Internal Server Error".to_string(),
                                        message: format!("Failed to add device subidentity: {}", err),
                                    }))
                                    .await;
                            }
                        }
                    }
                    IdentityType::Device => {
                        // use get_code_info to get the profile name
                        let code_info: shinkai_message_primitives::schemas::identity_registration::RegistrationCodeInfo = db.get_registration_code_info(code.clone().as_str()).unwrap();
                        let profile_name = match code_info.code_type {
                            RegistrationCodeType::Device(profile_name) => profile_name,
                            _ => return Err(Box::new(SqliteManagerError::InvalidData)),
                        };

                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();

                        // Check if the profile exists in the identity_manager
                        {
                            let mut identity_manager = identity_manager.lock().await;
                            let profile_identity_name = ShinkaiName::from_node_and_profile_names(
                                node_name.get_node_name_string(),
                                profile_name.clone(),
                            )
                            .unwrap();
                            if identity_manager
                                .find_by_identity_name(profile_identity_name.clone())
                                .is_none()
                            {
                                // If the profile doesn't exist, create and add it
                                let profile_identity = StandardIdentity {
                                    full_identity_name: profile_identity_name.clone(),
                                    addr: None,
                                    profile_encryption_public_key: Some(encryption_pk_obj),
                                    profile_signature_public_key: Some(signature_pk_obj),
                                    node_encryption_public_key: encryption_public_key,
                                    node_signature_public_key: identity_public_key,
                                    identity_type: StandardIdentityType::Profile,
                                    permission_type: IdentityPermissions::Admin,
                                };
                                identity_manager.add_profile_subidentity(profile_identity).await?;
                            }
                        }

                        // Logic for handling device identity
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(),
                        // profile_name.clone());
                        let full_identity_name = ShinkaiName::from_node_and_profile_names_and_type_and_name(
                            node_name.get_node_name_string(),
                            profile_name,
                            ShinkaiSubidentityType::Device,
                            registration_name.clone(),
                        )
                        .unwrap();

                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();

                        let device_signature_pk_obj =
                            string_to_signature_public_key(device_identity_pk.as_str()).unwrap();
                        let device_encryption_pk_obj =
                            string_to_encryption_public_key(device_encryption_pk.as_str()).unwrap();

                        let device_identity = DeviceIdentity {
                            full_identity_name: full_identity_name.clone(),
                            node_encryption_public_key: encryption_public_key,
                            node_signature_public_key: identity_public_key,
                            profile_encryption_public_key: encryption_pk_obj,
                            profile_signature_public_key: signature_pk_obj,
                            device_encryption_public_key: device_encryption_pk_obj,
                            device_signature_public_key: device_signature_pk_obj,
                            permission_type,
                        };

                        let api_v2_key = match db.read_api_v2_key() {
                            Ok(Some(api_key)) => api_key,
                            Ok(None) | Err(_) => {
                                let api_error = APIError {
                                    code: StatusCode::UNAUTHORIZED.as_u16(),
                                    error: "Unauthorized".to_string(),
                                    message: "Invalid bearer token".to_string(),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        };

                        let mut identity_manager_mut = identity_manager.lock().await;
                        match identity_manager_mut.add_device_subidentity(device_identity).await {
                            Ok(_) => {
                                std::mem::drop(identity_manager_mut);
                                if !main_profile_exists && !initial_llm_providers.is_empty() {
                                    let profile = full_identity_name.extract_profile()?;
                                    for llm_provider in &initial_llm_providers {
                                        Self::internal_add_llm_provider(
                                            db.clone(),
                                            identity_manager.clone(),
                                            job_manager.clone(),
                                            identity_secret_key.clone(),
                                            llm_provider.clone(),
                                            &profile,
                                            ws_manager.clone(),
                                        )
                                        .await?;
                                    }
                                }

                                if !first_device_needs_registration_code && !main_profile_exists {
                                    // Call the new function to scan and add Ollama models
                                    if let Err(err) = Self::scan_and_add_ollama_models(
                                        db.clone(),
                                        identity_manager.clone(),
                                        job_manager.clone(),
                                        identity_secret_key.clone(),
                                        node_name.clone(),
                                        ws_manager.clone(),
                                    )
                                    .await
                                    {
                                        error!("Failed to scan and add Ollama models: {}", err);
                                        // Note: We're not failing the entire operation if this fails
                                    }
                                }

                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: node_name.get_node_name_string().clone(),
                                    encryption_public_key: encryption_public_key_to_string(encryption_public_key),
                                    identity_public_key: signature_public_key_to_string(identity_public_key),
                                    api_v2_key,
                                    api_v2_cert: public_https_certificate,
                                };
                                let _ = res.send(Ok(success_response)).await.map_err(|_| ());
                            }
                            Err(err) => {
                                error!("Failed to add device subidentity: {}", err);
                                let _ = res
                                    .send(Err(APIError {
                                        code: StatusCode::BAD_REQUEST.as_u16(),
                                        error: "Internal Server Error".to_string(),
                                        message: format!("Failed to add device subidentity: {}", err),
                                    }))
                                    .await;
                            }
                        }
                    }
                    _ => {
                        // Handle other cases if required.
                    }
                }
            }
            Err(err) => {
                error!("Failed to add subidentity: {}", err);
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add device subidentity: {}", err),
                    }))
                    .await;
            }
        }
        Ok(())
    }

    async fn scan_and_add_ollama_models(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        node_name: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // if IS_TESTING then don't scan for ollama models
        let is_testing = env::var("IS_TESTING").unwrap_or_default();
        if is_testing == "true" || is_testing == "1" {
            return Ok(());
        }

        // Scan Ollama models
        let ollama_models = match Self::internal_scan_ollama_models().await {
            Ok(models) => models,
            Err(err) => {
                error!("Failed to scan Ollama models: {}", err);
                return Ok(()); // Continue even if scanning fails
            }
        };

        // Add Ollama models if any were found
        if !ollama_models.is_empty() {
            let models_to_add: Vec<String> = ollama_models
                .iter()
                .filter_map(|model| model["name"].as_str().map(String::from))
                .collect();

            if !models_to_add.is_empty() {
                let add_models_result = Self::internal_add_ollama_models(
                    db,
                    identity_manager,
                    job_manager,
                    identity_secret_key,
                    models_to_add,
                    node_name,
                    ws_manager,
                )
                .await;

                if let Err(err) = add_models_result {
                    error!("Failed to add Ollama models: {}", err);
                    // Note: We're not failing the entire operation if adding models fails
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_handle_send_onionized_message(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        identity_secret_key: SigningKey,
        potentially_encrypted_msg: ShinkaiMessage,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        if node_name.get_node_name_string().starts_with("@@localhost.") {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid node name: @@localhost".to_string(),
                }))
                .await;
            return Ok(());
        }

        let validation_result = Self::validate_message(
            encryption_secret_key.clone(),
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg.clone(),
            None,
        )
        .await;
        let (mut msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Error validating message: {}", api_error.message),
                    }))
                    .await;
                return Ok(());
            }
        };
        //
        // Part 2: Check if the message needs to be sent to another node or not
        //
        let recipient_node_name = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&msg.clone())
            .unwrap()
            .get_node_name_string();

        let sender_node_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&msg.clone())
            .unwrap()
            .get_node_name_string();

        if recipient_node_name == sender_node_name {
            //
            // Part 3A: Validate and store message locally
            //

            // Has sender access to the inbox specified in the message?
            let inbox = InboxName::from_message(&msg.clone());
            match inbox {
                Ok(inbox) => {
                    // TODO: extend and verify that the sender may have access to the inbox using the access db method
                    match inbox.has_sender_creation_access(msg.clone()) {
                        Ok(_) => {
                            // use unsafe_insert_inbox_message because we already validated the message
                            let parent_message_id = match msg.get_message_parent_key() {
                                Ok(key) => Some(key),
                                Err(_) => None,
                            };

                            db.unsafe_insert_inbox_message(&msg.clone(), parent_message_id, ws_manager.clone())
                                .await
                                .map_err(|e| {
                                    shinkai_log(
                                        ShinkaiLogOption::DetailedAPI,
                                        ShinkaiLogLevel::Error,
                                        format!("Error inserting message into db: {}", e).as_str(),
                                    );
                                    std::io::Error::new(std::io::ErrorKind::Other, format!("Insertion error: {}", e))
                                })?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::DetailedAPI,
                                ShinkaiLogLevel::Error,
                                format!("Error checking if sender has access to inbox: {}", e).as_str(),
                            );
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Bad Request".to_string(),
                                    message: format!("Error checking if sender has access to inbox: {}", e),
                                }))
                                .await;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("handle_onionized_message > Error getting inbox from message: {}", e);
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Error getting inbox from message: {}", e),
                        }))
                        .await;
                    return Ok(());
                }
            }
        }

        //
        // Part 3B: Preparing to externally send Message
        //
        // By default we encrypt all the messages between nodes. So if the message is not encrypted do it
        // we know the node that we want to send the message to from the recipient profile name
        let recipient_node_name_string = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&msg.clone())
            .unwrap()
            .to_string();

        let external_global_identity_result = identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_node_name_string.clone(), None)
            .await;

        let external_global_identity = match external_global_identity_result {
            Ok(identity) => identity,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Error".to_string(),
                        message: err,
                    }))
                    .await;
                return Ok(());
            }
        };

        msg.external_metadata.intra_sender = "".to_string();
        msg.encryption = EncryptionMethod::DiffieHellmanChaChaPoly1305;

        let encrypted_msg = msg.encrypt_outer_layer(
            &encryption_secret_key.clone(),
            &external_global_identity.node_encryption_public_key,
        )?;

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&identity_secret_key);
        let encrypted_msg = encrypted_msg.sign_outer_layer(&signature_sk)?;
        let node_addr = external_global_identity.addr.unwrap();

        Node::send(
            encrypted_msg,
            Arc::new(clone_static_secret_key(&encryption_secret_key)),
            (node_addr, recipient_node_name_string),
            proxy_connection_info,
            db.clone(),
            identity_manager.clone(),
            ws_manager.clone(),
            true,
            None,
        );

        {
            let inbox_name = match InboxName::from_message(&msg.clone()) {
                Ok(inbox) => inbox.to_string(),
                Err(_) => "".to_string(),
            };

            let scheduled_time = msg.external_metadata.scheduled_time;
            let message_hash = potentially_encrypted_msg.calculate_message_hash_for_pagination();

            let parent_key = if !inbox_name.is_empty() {
                match db.get_parent_message_hash(&inbox_name, &message_hash) {
                    Ok(result) => result,
                    Err(_) => None,
                }
            } else {
                None
            };

            let response = SendResponseBodyData {
                message_id: message_hash,
                parent_message_id: parent_key,
                inbox: inbox_name,
                scheduled_time,
            };

            if res.send(Ok(response)).await.is_err() {
                eprintln!("Failed to send response");
            }
        }

        Ok(())
    }
}
