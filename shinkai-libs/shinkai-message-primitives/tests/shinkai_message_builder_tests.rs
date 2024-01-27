#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use ed25519_dalek::SigningKey;
    use shinkai_message_primitives::{shinkai_message::{shinkai_message::{MessageBody, MessageData}, shinkai_message_schemas::JobMessage}, shinkai_utils::{encryption::{string_to_encryption_public_key, string_to_encryption_static_key, unsafe_deterministic_encryption_keypair, EncryptionMethod}, shinkai_message_builder::ShinkaiMessageBuilder, signatures::{string_to_signature_secret_key, unsafe_deterministic_signature_keypair}}};
    use x25519_dalek::{PublicKey, StaticSecret};
    use super::*;


    #[test]
    fn test_job_message() {
        let my_encryption_sk = string_to_encryption_static_key("b01a8082dba0a866fa82d8d3e2dea25b053387e2ac06f35a5e237104de2c5374").unwrap();
        let my_signature_sk = string_to_signature_secret_key("03b1cd8cdce9a8a54ce73a262f11c3ff17eedf9f696f14e9ab55a89476b22306").unwrap();
        let receiver_public_key = string_to_encryption_public_key("798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679").unwrap();

        let node_sender = "@@localhost.shinkai".to_string();
        let node_receiver = "@@localhost.shinkai".to_string();
        let sender_subidentity = "main".to_string();
        let scheduled_time = "2024-01-26T06:57:10.521Z".to_string();
        let inbox = "jobid_399c5571-3504-4aa7-a291-b1e086c1440c".to_string();
        let message_raw_content = "hello hello, are u there?".to_string();

        let message_result = ShinkaiMessageBuilder::job_message(
            inbox.clone(),
            message_raw_content.clone(),
            "".to_string(),
            "".to_string(),
            my_encryption_sk.clone(),
            my_signature_sk.clone(),
            receiver_public_key.clone(),
            node_sender.clone(),
            sender_subidentity.clone(),
            node_receiver.clone(),
            "".to_string(),
        );

        assert!(message_result.is_ok());
        let message = message_result.unwrap();

        // Check if the message content is as expected
        if let MessageBody::Unencrypted(shinkai_body) = &message.body {
            if let MessageData::Unencrypted(shinkai_data) = &shinkai_body.message_data {
                let job_message: JobMessage = serde_json::from_str(&shinkai_data.message_raw_content).unwrap();
                assert_eq!(job_message.job_id, inbox);
                assert_eq!(job_message.content, "hello hello, are u there?");
                assert_eq!(job_message.files_inbox, "");
            }
        }

        // Check if the external metadata is as expected
        assert_eq!(message.external_metadata.sender, node_sender);
        assert_eq!(message.external_metadata.recipient, node_receiver);

        // Check if the encryption method is as expected
        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);
    }
}