use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, JobScope, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use shinkai_node::schemas::identity::{Identity, IdentityType, StandardIdentity};
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub async fn api_registration_device_node_profile_main(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_encryption_pk: EncryptionPublicKey,
    device_encryption_sk: EncryptionStaticKey,
    device_signature_sk: SignatureStaticKey,
    profile_encryption_sk: EncryptionStaticKey,
    profile_signature_sk: SignatureStaticKey,
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

        eprintln!("code_message: {:?}", code_message);

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
            Ok(code) => assert_eq!(code, "true".to_string(), "{} used registration code", node_profile_name),
            Err(e) => panic!("Registration code error: {:?}", e),
        }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndAgents(
                res_all_subidentities_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();
        eprintln!("node2_all_subidentities: {:?}", node2_all_subidentities);

        assert_eq!(node2_all_subidentities.len(), 2, "Node has 1 subidentity");
        eprintln!(
            "{}",
            format!(
                "{} subidentity: {:?}",
                node_profile_name,
                node2_all_subidentities[0].get_full_identity_name()
            )
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
    subidentity_signature_sk: SignatureStaticKey,
    identities_number: usize,
) {
    {
        let permissions = IdentityPermissions::Admin;
        let code_type = RegistrationCodeType::Profile;

        let msg = ShinkaiMessageBuilder::request_code_registration(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk.clone(),
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

        eprintln!("code_message: {:?}", code_message);

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
            Ok(code) => assert_eq!(code, "true".to_string(), "{} used registration code", node_profile_name),
            Err(e) => panic!("Registration code error: {:?}", e),
        }

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
            "{}",
            format!(
                "{} subidentity: {:?}",
                node_profile_name, node2_all_subidentities[0].full_identity_name
            )
        );
        assert_eq!(
            node2_all_subidentities[identities_number - 1].full_identity_name,
            ShinkaiName::from_node_and_profile(node_identity_name.to_string(), node_profile_name.to_string()).unwrap(),
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
    node_subidentity_sk: SignatureStaticKey,
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

pub async fn api_agent_registration(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SignatureStaticKey,
    node_name: &str,
    subidentity_name: &str,
    agent: SerializedAgent,
) {
    {
        let code_message = ShinkaiMessageBuilder::request_add_agent(
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            agent.clone(),
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
        let node_agent_registration = res_agent_registration_receiver.recv().await.unwrap();

        eprintln!("code_message: {:?}", node_agent_registration);
        tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndAgents(
                res_all_subidentities_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();
        eprintln!("node2_all_subidentities: {:?}", node2_all_subidentities);

        // Search in node2_all_subidentities for the agent
        let agent_identity = node2_all_subidentities.iter().find(|identity| {
            identity.get_full_identity_name()
                == ShinkaiName::new(format!("{}/main/agent/{}", node_name, agent.id))
                    .unwrap()
                    .to_string()
        });

        assert!(agent_identity.is_some(), "Agent was added to the node");

        let available_agents_msg = ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
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
        eprintln!("available_agents_msg: {:?}", available_agents_msg);

        let (res_available_agents_sender, res_available_agents_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIAvailableAgents {
                msg: available_agents_msg.clone(),
                res: res_available_agents_sender,
            })
            .await
            .unwrap();
        let available_agents = res_available_agents_receiver.recv().await.unwrap();

        // Check if the result is Ok and extract the agents
        if let Ok(agents) = &available_agents {
            // Extract the agent IDs from the available agents
            let available_agent_ids: Vec<String> = agents.iter().map(|agent| agent.id.clone()).collect();

            // Check if the added agent's ID is in the list of available agent IDs
            assert!(available_agent_ids.contains(&agent.id), "Agent is not available");
        } else {
            panic!("Failed to get available agents");
        }
    }
}

pub async fn api_create_job(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SignatureStaticKey,
    sender: &str,
    sender_subidentity: &str,
    recipient_subidentity: &str,
) -> String {
    {
        let job_scope = JobScope {
            buckets: vec![],
            documents: vec![],
        };

        let full_sender = format!("{}/{}", sender, sender_subidentity);
        eprintln!("@@ full_sender: {}", full_sender);

        let job_creation = ShinkaiMessageBuilder::job_creation(
            job_scope,
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            full_sender.to_string(),
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

        return node_job_creation.unwrap();
    }
}

pub async fn api_message_job(
    node_commands_sender: Sender<NodeCommand>,
    subidentity_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    subidentity_signature_sk: SignatureStaticKey,
    sender: &str,
    sender_subidentity: &str,
    recipient_subidentity: &str,
    job_id: &str,
    content: &str,
) {
    {
        let full_sender = format!("{}/{}", sender, sender_subidentity);
        eprintln!("@@ full_sender: {}", full_sender);

        let job_message = ShinkaiMessageBuilder::job_message(
            job_id.to_string(),
            content.to_string(),
            subidentity_encryption_sk.clone(),
            clone_signature_secret_key(&subidentity_signature_sk),
            node_encryption_pk,
            full_sender.to_string(),
            sender.to_string(),
            recipient_subidentity.to_string(),
        )
        .unwrap();

        let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::APIJobMessage {
                msg: job_message,
                res: res_message_job_sender,
            })
            .await
            .unwrap();
        let node_job_message = res_message_job_receiver.recv().await.unwrap();
        eprintln!("node_job_message: {:?}", node_job_message);

        assert!(node_job_message.is_ok(), "Job message was successfully processed");
    }
}
