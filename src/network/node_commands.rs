use super::{
    external_identities::external_identity_to_profile_data,
    node_message_handlers::{
        extract_recipient_node_profile_name, extract_sender_node_profile_name, get_sender_keys,
        verify_message_signature,
    },
    ExternalProfileData, Node, RegistrationCode,
};
use crate::{
    network::Subidentity,
    shinkai_message::{
        encryption::{decrypt_message, string_to_encryption_public_key},
        shinkai_message_handler::{self, ShinkaiMessageHandler},
        signatures::{clone_signature_secret_key, string_to_signature_public_key},
    },
    shinkai_message_proto::ShinkaiMessage,
};
use async_channel::Sender;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{debug, error, info, trace, warn};
use std::{
    io::{self, Error},
    net::SocketAddr,
    time::Duration,
};
use tokio::sync::oneshot::error;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn send_peer_addresses(&self, sender: Sender<Vec<SocketAddr>>) -> Result<(), Error> {
        let peer_addresses: Vec<SocketAddr> =
            self.peers.clone().into_iter().map(|(k, _)| k.0).collect();
        sender.send(peer_addresses).await.unwrap();
        Ok(())
    }

    pub async fn handle_external_profile_data(
        &self,
        name: String,
        res: Sender<ExternalProfileData>,
    ) -> Result<(), Error> {
        let external_profile_data = external_identity_to_profile_data(name).unwrap();
        res.send(external_profile_data).await.unwrap();
        Ok(())
    }

    pub async fn connect_node(
        &self,
        address: SocketAddr,
        profile_name: String,
    ) -> Result<(), Error> {
        let address_str = address.to_string();
        self.connect(&address_str, profile_name).await?;
        Ok(())
    }

    // And so on for the rest of the methods...

    pub async fn handle_wrapped_message(&self, msg: ShinkaiMessage) -> Result<(), Error> {
        // check that the message is coming from a subidentity, sender needs to match a subidentity profile name
        // check that the signature is valid
        // decrypt the message (if it's encrypted)
        // re-encrypt the message and re-sign it with the node identity

        println!("handle_wrapped_message msg: {:?}", msg);

        let subidentity_manager = self.subidentity_manager.lock().await;

        // debug code
        let all_subidentities = subidentity_manager.get_all_subidentities();
        println!(
            "handle_wrapped_message all_subidentities: {:?}",
            all_subidentities
        );
        // end debug code

        let subidentity = subidentity_manager
            .find_by_profile_name(&msg.external_metadata.clone().unwrap().sender);

        println!("handle_wrapped_message subidentity: {:?}", subidentity);
        // check if subidentity exist
        if subidentity.is_none() {
            eprintln!(
                "Subidentity not found for profile name: {}",
                msg.external_metadata.clone().unwrap().sender
            );
            // TODO: add error messages here
            return Ok(());
        }

        // If we reach this point, it means that subidentity exists, so it's safe to unwrap
        // let subidentity = subidentity.unwrap();

        // Validate it
        let sender_keys = get_sender_keys(&msg)?;
        match verify_message_signature(sender_keys.signature_public_key, &msg) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to verify message signature: {}", e);
                return Ok(());
            }
        }

        // Save to db
        {
            let db = self.db.lock().await;
            db.insert_message(&msg.clone())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        let recipient_profile_name_string = extract_recipient_node_profile_name(&msg);
        let external_profile_data =
            external_identity_to_profile_data(recipient_profile_name_string.clone()).unwrap();

        let db_guard = self.db.lock().await;
        Node::send(
            &msg,
            (external_profile_data.addr, recipient_profile_name_string),
            &*db_guard,
        )
        .await?;

        Ok(())
    }

    pub async fn handle_unchanged_message(&self, msg: ShinkaiMessage) -> Result<(), Error> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        let subidentity_manager = self.subidentity_manager.lock().await;
        let subidentity = subidentity_manager
            .find_by_profile_name(&msg.external_metadata.clone().unwrap().sender);
        // check if subidentity exist
        if subidentity.is_none() {
            eprintln!(
                "Subidentity not found for profile name: {}",
                msg.external_metadata.clone().unwrap().sender
            );
            // TODO: add error messages here
            return Ok(());
        }

        // If we reach this point, it means that subidentity exists, so it's safe to unwrap
        let subidentity = subidentity.unwrap();

        // Validate it
        let signature_public_key = subidentity.signature_public_key.clone();
        if signature_public_key.is_none() {
            eprintln!(
                "Signature public key doesn't exist for identity: {}",
                subidentity.name.clone()
            );
            // TODO: add error messages here
            return Ok(());
        }

        match verify_message_signature(signature_public_key.unwrap(), &msg) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to verify message signature: {}", e);
                return Ok(());
            }
        }

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&self.identity_secret_key);
        let msg = ShinkaiMessageHandler::re_sign_message(msg, signature_sk);
        // Save to db
        {
            let db = self.db.lock().await;
            db.insert_message(&msg.clone())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        let recipient_profile_name_string = extract_recipient_node_profile_name(&msg);
        let external_profile_data =
            external_identity_to_profile_data(recipient_profile_name_string.clone()).unwrap();

        let db_guard = self.db.lock().await;
        Node::send(
            &msg,
            (external_profile_data.addr, recipient_profile_name_string),
            &*db_guard,
        )
        .await?;
        // println!(
        //     "handle_unchanged_message who am I: {:?}",
        //     self.node_profile_name
        // );
        // println!(
        //     "Finished successfully> handle_unchanged_message msg: {:?}",
        //     msg
        // );
        // println!("\n\n");
        Ok(())
    }

    pub async fn send_public_keys(
        &self,
        res: Sender<(SignaturePublicKey, EncryptionPublicKey)>,
    ) -> Result<(), Error> {
        let identity_public_key = self.identity_public_key.clone();
        let encryption_public_key = self.encryption_public_key.clone();
        let _ = res
            .send((identity_public_key, encryption_public_key))
            .await
            .map_err(|_| ());
        Ok(())
    }

    pub async fn fetch_and_send_last_messages(
        &self,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    ) -> Result<(), Error> {
        let db = self.db.lock().await;
        let messages = db.get_last_messages(limit).unwrap_or_else(|_| vec![]);
        let _ = res.send(messages).await.map_err(|_| ());
        Ok(())
    }

    pub async fn create_and_send_registration_code(
        &self,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.db.lock().await;
        let code = db
            .generate_registration_new_code()
            .unwrap_or_else(|_| "".to_string());
        let _ = res.send(code).await.map_err(|_| ());
        Ok(())
    }

    pub async fn handle_registration_code_usage(
        &self,
        msg: ShinkaiMessage,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sender_encryption_pk_string = msg.external_metadata.clone().unwrap().other;
        let sender_encryption_pk =
            string_to_encryption_public_key(sender_encryption_pk_string.as_str()).unwrap();

        // Decrypt the message
        let decrypted_message_result = decrypt_message(
            &msg.clone(),
            &self.encryption_secret_key,
            &sender_encryption_pk, // from the other field in external_metadata
        );

        // You'll need to handle the case where decryption fails
        let decrypted_message = match decrypted_message_result {
            Ok(message) => message,
            Err(_) => {
                // TODO: add more debug info
                println!("Failed to decrypt the message");
                return Ok(());
            }
        };

        // Deserialize body.content into RegistrationCode
        let content = decrypted_message.body.clone().unwrap().content;
        let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();

        // Extract values from the ShinkaiMessage
        let code = registration_code.code;
        let profile_name = registration_code.profile_name;
        let identity_pk = registration_code.identity_pk;
        let encryption_pk = registration_code.encryption_pk;

        let db = self.db.lock().await;
        let result = db
            .use_registration_code(&code, &profile_name, &identity_pk, &encryption_pk)
            .map_err(|e| e.to_string())
            .map(|_| "true".to_string());
        std::mem::drop(db);

        // TODO: add code if you are the first one some special stuff happens.
        // definition of a shared symmetric encryption key
        // probably we need to sign a message with the pk from the first user
        match result {
            Ok(success) => {
                let signature_pk_obj =
                    string_to_signature_public_key(identity_pk.as_str()).unwrap();
                let encryption_pk_obj =
                    string_to_encryption_public_key(encryption_pk.as_str()).unwrap();

                let subidentity = Subidentity {
                    name: profile_name.clone(),
                    addr: None,
                    signature_public_key: Some(signature_pk_obj),
                    encryption_public_key: Some(encryption_pk_obj),
                };
                let mut subidentity_manager = self.subidentity_manager.lock().await;
                match subidentity_manager.add_subidentity(subidentity).await {
                    Ok(_) => {
                        let _ = res.send(success).await.map_err(|_| ());
                    }
                    Err(err) => {
                        error!("Failed to add subidentity: {}", err);
                    }
                }
            }
            Err(e) => {
                error!("Failed to add subidentity: {}", e);
                let _ = res.send(e).await.map_err(|_| ());
            }
        }
        Ok(())
    }

    pub async fn get_all_subidentities(
        &self,
        res: Sender<Vec<Subidentity>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let subidentity_manager = self.subidentity_manager.lock().await;
        let subidentities = subidentity_manager.get_all_subidentities();
        let _ = res.send(subidentities).await.map_err(|_| ());
        Ok(())
    }
}
