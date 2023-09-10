use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType,
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

pub async fn local_registration_profile_node(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_profile_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    node_subidentity_sk: SignatureStaticKey,
    identities_number: usize,
) {
    {
        let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::LocalCreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Profile,
                res: res_registration_sender,
            })
            .await
            .unwrap();
        let node_registration_code = res_registraton_receiver.recv().await.unwrap();

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

        // use GetAllSubidentitiesDevicesAndAgents to check if the subidentity is registered
        let (res_all_subidentities_devices_and_agents_sender, res_all_subidentities_devices_and_agents_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndAgents(
                res_all_subidentities_devices_and_agents_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities_devices_and_agents = res_all_subidentities_devices_and_agents_receiver
            .recv()
            .await
            .unwrap()
            .unwrap();

        eprintln!(
            "{}",
            format!(
                "{} subidentity: {:?}",
                node_profile_name,
                node2_all_subidentities_devices_and_agents[0].get_full_identity_name()
            )
        );
    }
}