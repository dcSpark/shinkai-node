use async_channel::Sender;
use core::panic;
use ed25519_dalek::SigningKey;
use serde_json::{Map, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::identity::{Identity, IdentityType, StandardIdentity};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType
};
use shinkai_message_primitives::shinkai_utils::encryption::encryption_public_key_to_string;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use std::time::Duration;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[allow(clippy::too_many_arguments)]
pub async fn api_registration_device_node_profile_main(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_encryption_pk: EncryptionPublicKey,
    device_encryption_sk: EncryptionStaticKey,
    device_signature_sk: SigningKey,
    profile_encryption_sk: EncryptionStaticKey,
    profile_signature_sk: SigningKey,
    device_name_for_profile: &str,
) {
    {
        let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::LocalCreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Device("main".to_string()),
                res: res_registration_sender,
            })
            .await
            .unwrap();
        let node_registration_code = res_registraton_receiver.recv().await.unwrap();

        let code_message = ShinkaiMessageBuilder::use_code_registration_for_device(
            device_encryption_sk.clone(),
            clone_signature_secret_key(&device_signature_sk),
            profile_encryption_sk.clone(),
            clone_signature_secret_key(&profile_signature_sk),
            node_encryption_pk,
            node_registration_code.to_string(),
            IdentityType::Device.to_string(),
            IdentityPermissions::Admin.to_string(),
            device_name_for_profile.to_string().clone(),
            "".to_string(),
            node_identity_name.to_string(),
            node_identity_name.to_string(),
        )
        .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_use_registration_sender, res_use_registraton_receiver) = async_channel::bounded(2);

        eprintln!("node_commands_sender: {:?}", node_commands_sender);
        eprintln!("res_use_registration_sender: {:?}", res_use_registration_sender);
        node_commands_sender
            .send(NodeCommand::APIUseRegistrationCode {
                msg: code_message,
                res: res_use_registration_sender,
            })
            .await
            .unwrap();
        let node2_use_registration_code = res_use_registraton_receiver.recv().await.unwrap();
        eprintln!("node_use_registration_code: {:?}", node2_use_registration_code);
        match node2_use_registration_code {
            Ok(code) => assert_eq!(
                code.message,
                "true".to_string(),
                "{} used registration code",
                node_profile_name
            ),
            Err(e) => panic!("Registration code error: {:?}", e),
        }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        #[allow(clippy::type_complexity)]
        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndLLMProviders(
                res_all_subidentities_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();
        eprintln!("node_all_subidentities: {:?}", node2_all_subidentities);
        shinkai_log(
            ShinkaiLogOption::Tests,
            ShinkaiLogLevel::Debug,
            format!(
                "{} subidentity: {:?}",
                node_profile_name,
                node2_all_subidentities[0].get_full_identity_name()
            )
            .as_str(),
        );
        assert_eq!(
            node2_all_subidentities[1].get_full_identity_name(),
            format!("{}/main/device/{}", node_identity_name, device_name_for_profile),
            "Node has the right subidentity"
        );
    }
}

pub async fn api_registration_profile_node(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    identities_number: usize,
) {
    {
        let permissions = IdentityPermissions::Admin;
        let code_type = RegistrationCodeType::Profile;

        let msg = ShinkaiMessageBuilder::request_code_registration(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            permissions,
            code_type,
            "main".to_string().clone(),
            node_identity_name.to_string().clone(),
            node_identity_name.to_string().clone(),
        )
        .expect("Failed to create registration message");

        eprintln!("Msg: {:?}", msg);

        let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APICreateRegistrationCode {
                msg,
                res: res_registration_sender,
            })
            .await
            .unwrap();
        let node_registration_code = match res_registraton_receiver.recv().await {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error receiving node registration code: {:?}", e);
                panic!("Error receiving node registration code: {:?}", e);
            }
        };

        eprintln!("node_registration_code: {:?}", node_registration_code);

        let code_message = ShinkaiMessageBuilder::use_code_registration_for_profile(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            node_registration_code.unwrap().to_string(),
            IdentityType::Profile.to_string(),
            IdentityPermissions::Admin.to_string(),
            node_profile_name.to_string().clone(),
            node_profile_name.to_string().clone(),
            node_identity_name.to_string(),
            node_identity_name.to_string(),
        )
        .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_use_registration_sender, res_use_registraton_receiver) = async_channel::bounded(2);

        eprintln!("node_commands_sender: {:?}", node_commands_sender);
        eprintln!("res_use_registration_sender: {:?}", res_use_registration_sender);
        node_commands_sender
            .send(NodeCommand::APIUseRegistrationCode {
                msg: code_message,
                res: res_use_registration_sender,
            })
            .await
            .unwrap();
        let node2_use_registration_code = res_use_registraton_receiver.recv().await.unwrap();
        eprintln!("node2_use_registration_code: {:?}", node2_use_registration_code);
        match node2_use_registration_code {
            Ok(code) => assert_eq!(
                code.message,
                "true".to_string(),
                "{} used registration code",
                node_profile_name
            ),
            Err(e) => panic!("Registration code error: {:?}", e),
        }

        #[allow(clippy::type_complexity)]
        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<StandardIdentity>, APIError>>,
            async_channel::Receiver<Result<Vec<StandardIdentity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIGetAllSubidentities {
                res: res_all_subidentities_sender,
            })
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();

        assert_eq!(
            node2_all_subidentities.len(),
            identities_number,
            "Node has 1 subidentity"
        );
        eprintln!(
            "{} subidentity: {:?}",
            node_profile_name, node2_all_subidentities[0].full_identity_name
        );
        assert_eq!(
            node2_all_subidentities[identities_number - 1].full_identity_name,
            ShinkaiName::from_node_and_profile_names(node_identity_name.to_string(), node_profile_name.to_string())
                .unwrap(),
            "Node has the right subidentity"
        );
    }
}

pub async fn api_try_re_register_profile_node(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_profile_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    node_subidentity_sk: SigningKey,
) {
    let (res1_registration_sender, res1_registraton_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::LocalCreateRegistrationCode {
            permissions: IdentityPermissions::Admin,
            code_type: RegistrationCodeType::Profile,
            res: res1_registration_sender,
        })
        .await
        .unwrap();
    let node_registration_code = res1_registraton_receiver.recv().await.unwrap();

    let code_message = ShinkaiMessageBuilder::use_code_registration_for_profile(
        node_profile_encryption_sk.clone(),
        clone_signature_secret_key(&node_subidentity_sk),
        node_encryption_pk,
        node_registration_code.to_string(),
        IdentityType::Profile.to_string(),
        IdentityPermissions::Admin.to_string(),
        node_profile_name.to_string().clone(),
        node_profile_name.to_string().clone(),
        node_identity_name.to_string(),
        node_identity_name.to_string(),
    )
    .unwrap();

    let (res1_use_registration_sender, res1_use_registraton_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIUseRegistrationCode {
            msg: code_message,
            res: res1_use_registration_sender,
        })
        .await
        .unwrap();
    let node1_use_registration_code = res1_use_registraton_receiver.recv().await.unwrap();
    match node1_use_registration_code {
        Ok(_) => panic!("Registration passed. It shouldn't! Profile should already exists"),
        Err(e) => match e {
            APIError {
                code: 400,
                error: _,
                message,
            } if message == "Failed to add device subidentity: Profile name already exists" => (),
            _ => panic!("Registration code error: {:?}", e),
        },
    }

    #[allow(clippy::type_complexity)]
    let (res1_all_subidentities_sender, res1_all_subidentities_receiver): (
        async_channel::Sender<Result<Vec<StandardIdentity>, APIError>>,
        async_channel::Receiver<Result<Vec<StandardIdentity>, APIError>>,
    ) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIGetAllSubidentities {
            res: res1_all_subidentities_sender,
        })
        .await
        .unwrap();
    let node1_all_subidentities = res1_all_subidentities_receiver.recv().await.unwrap();
    assert_eq!(
        node1_all_subidentities.unwrap().len(),
        1,
        "Node still has 1 subidentity"
    );
}

pub async fn api_llm_provider_registration(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    node_name: &str,
    subidentity_name: &str,
    llm_provider: SerializedLLMProvider,
) {
    {
        let code_message = ShinkaiMessageBuilder::request_add_llm_provider(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            llm_provider.clone(),
            subidentity_name.to_string(),
            node_name.to_string(),
            node_name.to_string(),
        )
        .unwrap();

        let (res_agent_registration_sender, res_agent_registration_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIAddAgent {
                msg: code_message,
                res: res_agent_registration_sender,
            })
            .await
            .unwrap();
        let _node_agent_registration = res_agent_registration_receiver.recv().await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        #[allow(clippy::type_complexity)]
        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndLLMProviders(
                res_all_subidentities_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();

        // Search in node2_all_subidentities for the agent
        let agent_identity = node2_all_subidentities.iter().find(|identity| {
            identity.get_full_identity_name()
                == ShinkaiName::new(format!("{}/main/agent/{}", node_name, llm_provider.id))
                    .unwrap()
                    .to_string()
        });

        assert!(agent_identity.is_some(), "Agent was added to the node");

        let available_llm_providers_msg = ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            "".to_string(),
            subidentity_name.to_string(),
            node_name.to_string(),
            node_name.to_string(),
            MessageSchemaType::Empty,
        )
        .unwrap();
        eprintln!("available_llm_providers_msg: {:?}", available_llm_providers_msg);

        let (res_available_llm_providers_sender, res_available_llm_providers_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIAvailableLLMProviders {
                msg: available_llm_providers_msg.clone(),
                res: res_available_llm_providers_sender,
            })
            .await
            .unwrap();
        let available_llm_providers = res_available_llm_providers_receiver.recv().await.unwrap();

        // Check if the result is Ok and extract the llm providers
        if let Ok(llm_providers) = &available_llm_providers {
            // Extract the agent IDs from the available llm providers
            let available_llm_providers_ids: Vec<String> = llm_providers.iter().map(|agent| agent.id.clone()).collect();

            // Check if the added agent's ID is in the list of available agent IDs
            assert!(
                available_llm_providers_ids.contains(&llm_provider.id),
                "Agent is not available"
            );
        } else {
            panic!("Failed to get available llm providers");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn api_message_job(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    sender: &str,
    sender_subidentity: &str,
    recipient_subidentity: &str,
    job_id: &str,
    content: &str,
    files: &[&str],
    parent: &str,
) {
    {
        let files_vec = files.iter().map(|f| ShinkaiPath::new(f)).collect::<Vec<_>>();
        let job_message = ShinkaiMessageBuilder::job_message(
            job_id.to_string(),
            content.to_string(),
            files_vec,
            parent.to_string(),
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            sender.to_string(),
            sender_subidentity.to_string(),
            sender.to_string(),
            recipient_subidentity.to_string(),
        )
        .unwrap();

        let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIJobMessage {
                msg: job_message.clone(),
                res: res_message_job_sender,
            })
            .await
            .unwrap();
        let node_job_message = res_message_job_receiver.recv().await.unwrap();
        eprintln!("node_job_message: {:?}", node_job_message);

        assert!(node_job_message.is_ok(), "Job message was successfully processed");
    }
}

pub async fn api_create_job(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    sender: &str,
    sender_subidentity: &str,
    recipient_subidentity: &str,
) -> String {
    let job_scope = MinimalJobScope::default();
    api_create_job_with_scope(
        node_commands_sender,
        subidentity_encryption_sk,
        node_encryption_pk,
        subidentity_signature_sk,
        sender,
        sender_subidentity,
        recipient_subidentity,
        job_scope,
    )
    .await
}

pub async fn api_create_job_with_scope(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    sender: &str,
    sender_subidentity: &str,
    recipient_subidentity: &str,
    job_scope: MinimalJobScope,
) -> String {
    {
        let full_sender = format!("{}/{}", sender, sender_subidentity);
        eprintln!("@@ full_sender: {}", full_sender);

        let job_creation = ShinkaiMessageBuilder::job_creation(
            job_scope,
            false,
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            sender.to_string(),
            sender_subidentity.to_string(),
            sender.to_string(),
            recipient_subidentity.to_string(),
        )
        .unwrap();

        let (res_create_job_sender, res_create_job_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APICreateJob {
                msg: job_creation,
                res: res_create_job_sender,
            })
            .await
            .unwrap();
        let node_job_creation = res_create_job_receiver.recv().await.unwrap();
        eprintln!("node_job_creation: {:?}", node_job_creation);

        assert!(node_job_creation.is_ok(), "Job was created");

        node_job_creation.unwrap()
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn api_initial_registration_with_no_code_for_device(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_encryption_pk: EncryptionPublicKey,
    device_encryption_sk: EncryptionStaticKey,
    device_signature_sk: SigningKey,
    profile_encryption_sk: EncryptionStaticKey,
    profile_signature_sk: SigningKey,
    device_name_for_profile: &str,
) {
    let recipient = node_identity_name.to_string();
    let sender = recipient.clone();
    let sender_subidentity = "main".to_string();

    let message_result = ShinkaiMessageBuilder::initial_registration_with_no_code_for_device(
        device_encryption_sk.clone(),
        clone_signature_secret_key(&device_signature_sk),
        profile_encryption_sk.clone(),
        clone_signature_secret_key(&profile_signature_sk),
        device_name_for_profile.to_string(),
        sender_subidentity.clone(),
        sender.clone(),
        recipient.clone(),
    )
    .unwrap();

    let (res_use_registration_sender, res_use_registraton_receiver) = async_channel::bounded(2);

    node_commands_sender
        .send(NodeCommand::APIUseRegistrationCode {
            msg: message_result,
            res: res_use_registration_sender,
        })
        .await
        .unwrap();
    let node2_use_registration_code = res_use_registraton_receiver.recv().await.unwrap();
    match node2_use_registration_code {
        Ok(code) => {
            assert_eq!(
                code.message,
                "true".to_string(),
                "{} used registration code",
                node_profile_name
            );
            assert_eq!(
                code.encryption_public_key,
                encryption_public_key_to_string(node_encryption_pk),
                "{} used registration code",
                node_profile_name
            );
        }
        Err(e) => panic!("Registration code error: {:?}", e),
    }

    #[allow(clippy::type_complexity)]
    let (res_all_subidentities_sender, res_all_subidentities_receiver): (
        async_channel::Sender<Result<Vec<Identity>, APIError>>,
        async_channel::Receiver<Result<Vec<Identity>, APIError>>,
    ) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::GetAllSubidentitiesDevicesAndLLMProviders(
            res_all_subidentities_sender,
        ))
        .await
        .unwrap();
    let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();

    assert!(node2_all_subidentities.len() >= 2, "Node has 1 subidentity");
    assert_eq!(
        node2_all_subidentities[1].get_full_identity_name(),
        format!("{}/main/device/{}", node_identity_name, device_name_for_profile),
        "Node has the right subidentity"
    );
}

pub async fn api_get_all_inboxes_from_profile(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SigningKey,
    sender: &str,
    sender_subidentity: &str,
    recipient: &str,
) -> Vec<String> {
    {
        let inbox_message = ShinkaiMessageBuilder::get_all_inboxes_for_profile(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            sender_subidentity.to_string(),
            sender_subidentity.to_string(),
            sender.to_string(),
            recipient.to_string(),
        )
        .unwrap();
        eprintln!("inbox_message: {:?}", inbox_message);

        let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIGetAllInboxesForProfile {
                msg: inbox_message,
                res: res_message_job_sender,
            })
            .await
            .unwrap();
        let node_job_message = res_message_job_receiver.recv().await.unwrap();
        eprintln!("get all inboxes: {:?}", node_job_message);
        assert!(node_job_message.is_ok(), "Job message was successfully processed");
        node_job_message.unwrap()
    }
}

pub async fn api_execute_tool(
    node_commands_sender: Sender<NodeCommand>,
    bearer: String,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    agent_id: Option<String>,
    llm_provider: String,
    extra_config: Map<String, Value>,
    _oauth: Map<String, Value>,
) -> Result<Value, APIError> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    let mounts = None;
    node_commands_sender
        .send(NodeCommand::V2ApiExecuteTool {
            bearer,
            tool_router_key,
            parameters,
            tool_id,
            app_id,
            agent_id,
            llm_provider,
            extra_config,
            mounts,
            res: res_sender,
        })
        .await
        .unwrap();

    res_receiver.recv().await.unwrap()
}

pub async fn wait_for_default_tools(
    node_commands_sender: Sender<NodeCommand>,
    bearer: String,
    timeout_seconds: u64,
) -> Result<bool, APIError> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_seconds);

    while start.elapsed() < timeout {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::V2ApiCheckDefaultToolsSync {
                bearer: bearer.clone(),
                res: res_sender,
            })
            .await
            .unwrap();

        match res_receiver.recv().await.unwrap() {
            Ok(true) => return Ok(true),
            Ok(false) => {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(false) // Timeout reached without getting true
}

pub async fn wait_for_rust_tools(
    node_commands_sender: Sender<NodeCommand>,
    timeout_seconds: u64,
) -> Result<u32, String> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_seconds);
    let retry_delay = std::time::Duration::from_millis(500);
    let mut retry_count = 0;

    while start.elapsed() < timeout {
        tokio::time::sleep(retry_delay).await;

        let (res_sender, res_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::InternalCheckRustToolsInstallation { res: res_sender })
            .await
            .unwrap();

        match res_receiver.recv().await {
            Ok(result) => {
                match result {
                    Ok(has_tools) => {
                        if has_tools {
                            // Rust tools are installed, return the count of retries
                            return Ok(retry_count);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error checking Rust tools installation: {:?}", e);
                        return Err(format!("Error checking Rust tools installation: {:?}", e));
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Error receiving check result: {:?}", e);
                eprintln!("{}", error_msg);
                return Err(error_msg);
            }
        }

        retry_count += 1;
    }

    Err(format!(
        "Rust tools were not installed after {} seconds",
        timeout_seconds
    ))
}
