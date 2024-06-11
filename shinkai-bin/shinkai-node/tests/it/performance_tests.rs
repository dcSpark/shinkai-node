use ed25519_dalek::{SigningKey, VerifyingKey};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::{
        encryption::unsafe_deterministic_encryption_keypair, signatures::unsafe_deterministic_signature_keypair,
    },
};
use std::time::Instant;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn create_message(
    take_size: usize,
    node_identity_name: &str,
    node_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    node_identity_sk: SigningKey,
) -> ShinkaiMessage {
    let message_content = std::iter::repeat("a").take(take_size).collect::<String>();

    ShinkaiMessageBuilder::new(
        node_encryption_sk.clone(),
        clone_signature_secret_key(&node_identity_sk),
        node_encryption_pk,
    )
    .message_raw_content(message_content.clone())
    .no_body_encryption()
    .message_schema_type(MessageSchemaType::TextContent)
    .internal_metadata(
        "".to_string(),
        "".to_string(),
        EncryptionMethod::DiffieHellmanChaChaPoly1305,
        None,
    )
    .external_metadata_with_other(
        node_identity_name.to_string().clone(),
        node_identity_name.to_string().clone(),
        "".to_string(),
    )
    .build()
    .unwrap()
}

// #[test]
fn test_big_file_performance() {
    let node1_identity_name = "@@node1.shinkai";
    let _node2_identity_name = "@@node2.shinkai";

    let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (_node2_identity_sk, _node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_node2_encryption_sk, _node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    {
        let start = Instant::now();
        let take_size = 1048; // 1kb
        let _ = create_message(
            take_size,
            node1_identity_name,
            node1_encryption_sk.clone(),
            node1_encryption_pk,
            clone_signature_secret_key(&node1_identity_sk),
        );
        let duration = start.elapsed();
        println!(
            "Time elapsed in message creation for {}kb is: {:?}",
            (take_size / 1000),
            duration
        );
    }

    {
        let start = Instant::now();
        let take_size = 50000; // 50kb
        let _ = create_message(
            take_size,
            node1_identity_name,
            node1_encryption_sk.clone(),
            node1_encryption_pk,
            clone_signature_secret_key(&node1_identity_sk),
        );
        let duration = start.elapsed();
        println!(
            "Time elapsed in message creation for {}kb is: {:?}",
            (take_size / 1000),
            duration
        );
    }

    {
        let start = Instant::now();
        let take_size = 10_000_000; // 10mb
        let _ = create_message(
            take_size,
            node1_identity_name,
            node1_encryption_sk.clone(),
            node1_encryption_pk,
            clone_signature_secret_key(&node1_identity_sk),
        );
        let duration = start.elapsed();
        println!(
            "Time elapsed in message creation for {}kb is: {:?}",
            (take_size / 1000),
            duration
        );
    }

    // {
    //     let start = Instant::now();
    //     let take_size = 1_000_000_000; // 1gb
    //     let _ = create_message(
    //         take_size,
    //         node1_identity_name.clone(),
    //         node1_encryption_sk.clone(),
    //         node1_encryption_pk.clone(),
    //         clone_signature_secret_key(&node1_identity_sk),
    //     );
    //     let duration = start.elapsed();
    //     println!("Time elapsed in message creation for {}kb is: {:?}", (take_size / 1000), duration);
    // }
}
