// src/utils/cli.rs
use super::{args::Args, keys::NodeKeys};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message_schemas::MessageSchemaType,
    shinkai_utils::{
        encryption::{string_to_encryption_public_key, EncryptionMethod},
        shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
        signatures::clone_signature_secret_key,
    },
};
use x25519_dalek::PublicKey as EncryptionPublicKey;

pub fn cli_handle_create_message(args: Args, node_keys: &NodeKeys, global_identity_name: &str) {
    if args.create_message {
        let node2_encryption_pk_str = args
            .receiver_encryption_pk
            .expect("receiver_encryption_pk argument is required for create_message");
        let recipient = args
            .recipient
            .expect("recipient argument is required for create_message");
        let sender_subidentity = args.sender_subidentity.unwrap_or("".to_string());
        let receiver_subidentity = args.receiver_subidentity.unwrap_or("".to_string());
        let inbox = args.inbox.unwrap_or("".to_string());
        let body_content = args.body_content.unwrap_or("body content".to_string());
        let node2_encryption_pk = string_to_encryption_public_key(node2_encryption_pk_str.as_str()).unwrap();

        println!("Creating message for recipient: {}", recipient);
        println!("receiver_encryption_pk: {}", node2_encryption_pk_str);

        if let Some(code) = args.code_registration {
            println!("TODO: code_registration: {}", code);
            // handle_code_registration(code, node_keys, global_identity_name, recipient, node2_encryption_pk);
        } else {
            handle_create_message(
                node_keys,
                global_identity_name,
                recipient,
                node2_encryption_pk,
                sender_subidentity,
                receiver_subidentity,
                inbox,
                body_content,
            );
        }
    }
}

fn handle_create_message(
    node_keys: &NodeKeys,
    global_identity_name: &str,
    recipient: String,
    node2_encryption_pk: EncryptionPublicKey,
    sender_subidentity: ShinkaiNameString,
    receiver_subidentity: String,
    inbox: String,
    body_content: String,
) {
    let message = ShinkaiMessageBuilder::new(
        node_keys.encryption_secret_key.clone(),
        clone_signature_secret_key(&node_keys.identity_secret_key),
        node2_encryption_pk,
    )
    .message_raw_content(body_content.to_string())
    .body_encryption(EncryptionMethod::None)
    .message_schema_type(MessageSchemaType::Empty)
    .internal_metadata_with_inbox(
        sender_subidentity.to_string(),
        receiver_subidentity.to_string(),
        inbox.to_string(),
        EncryptionMethod::None,
        None,
    )
    .external_metadata(recipient.to_string(), global_identity_name.to_string().clone())
    .build();

    println!(
        "Message's signature: {}",
        message.clone().unwrap().external_metadata.signature
    );

    // Serialize the wrapper into JSON and print to stdout
    let message_json = serde_json::to_string_pretty(&message);

    match message_json {
        Ok(json) => println!("{}", json),
        Err(e) => println!("Error creating JSON: {}", e),
    }
}
