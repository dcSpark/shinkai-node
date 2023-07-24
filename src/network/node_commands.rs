use super::{node_message_handlers::verify_message_signature, Node};
use crate::{
    db::db_errors::ShinkaiDBError,
    managers::identity_manager::{self, Identity, IdentityManager, IdentityType, RegistrationCode},
    network::node_message_handlers::{ping_pong, PingPong},
    schemas::{
        inbox_permission::InboxPermission,
    },
    shinkai_message::{
        encryption::{
            clone_static_secret_key, decrypt_body_message, encryption_public_key_to_string,
            encryption_secret_key_to_string, string_to_encryption_public_key,
        },
        shinkai_message_handler::{self, ShinkaiMessageHandler},
        signatures::{clone_signature_secret_key, string_to_signature_public_key},
    },
    shinkai_message_proto::ShinkaiMessage,
};
use async_channel::Sender;
use chrono::{TimeZone, Utc};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{debug, error, info, trace, warn};
use std::str::FromStr;
use std::{
    cell::RefCell,
    io::{self, Error},
    net::SocketAddr,
};
use tokio::sync::oneshot::error;
use uuid::Uuid;
use warp::path::full;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn send_peer_addresses(&self, sender: Sender<Vec<SocketAddr>>) -> Result<(), Error> {
        let peer_addresses: Vec<SocketAddr> = self.peers.clone().into_iter().map(|(k, _)| k.0).collect();
        sender.send(peer_addresses).await.unwrap();
        Ok(())
    }

    pub async fn handle_external_profile_data(&self, name: String, res: Sender<Identity>) -> Result<(), Error> {
        let external_global_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&name)
            .await
            .unwrap();
        res.send(external_global_identity).await.unwrap();
        Ok(())
    }

    pub async fn connect_node(&self, address: SocketAddr, profile_name: String) -> Result<(), Error> {
        let address_str = address.to_string();
        self.connect(&address_str, profile_name).await?;
        Ok(())
    }

    pub async fn handle_onionized_message(&self, potentially_encrypted_msg: ShinkaiMessage) -> Result<(), Error> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        println!("handle_onionized_message msg: {:?}", potentially_encrypted_msg);

        let msg = if ShinkaiMessageHandler::is_body_currently_encrypted(&potentially_encrypted_msg.clone()) {
            // Decrypt the message
            let sender_encryption_pk_string = potentially_encrypted_msg
                .clone()
                .external_metadata
                .clone()
                .unwrap()
                .other;
            let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).unwrap();

            let decrypted_msg = decrypt_body_message(
                &potentially_encrypted_msg.clone(),
                &self.encryption_secret_key,
                &sender_encryption_pk,
            );

            match decrypted_msg {
                Ok(msg) => msg,
                Err(e) => {
                    println!("handle_onionized_message > Error decrypting message: {}", e);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Decryption error: {}", e),
                    ));
                }
            }
        } else {
            potentially_encrypted_msg.clone()
        };

        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = msg
            .clone()
            .body
            .unwrap()
            .internal_metadata
            .unwrap()
            .sender_subidentity
            .clone();

        let subidentity = subidentity_manager.find_by_profile_name(&sender_subidentity).cloned();
        std::mem::drop(subidentity_manager);

        // Check that the subidentity that's trying to prox through us exist / is valid
        if subidentity.is_none() {
            eprintln!(
                "handle_onionized_message > Subidentity not found for profile name: {}",
                msg.external_metadata.clone().unwrap().sender
            );
            // TODO: add error messages here
            return Ok(());
        }

        // If we reach this point, it means that subidentity exists, so it's safe to unwrap
        let subidentity = subidentity.unwrap();

        // Validate that the message actually came from the subidentity
        let signature_public_key = subidentity.subidentity_signature_public_key.clone();
        if signature_public_key.is_none() {
            eprintln!(
                "handle_onionized_message > Signature public key doesn't exist for identity: {}",
                subidentity.full_identity_name.clone()
            );
            // TODO: add error messages here
            return Ok(());
        }

        match verify_message_signature(signature_public_key.unwrap(), &potentially_encrypted_msg.clone()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("handle_onionized_message > Failed to verify message signature: {}", e);
                return Ok(());
            }
        }

        // By default we encrypt all the messages between nodes. So if the message is not encrypted do it
        // we know the node that we want to send the message to from the recipient profile name
        let recipient_node_profile_name = msg.external_metadata.clone().unwrap().recipient;
        println!(
            "handle_onionized_message > recipient_node_profile_name: {}",
            recipient_node_profile_name
        );

        let external_global_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_node_profile_name.clone())
            .await
            .unwrap();

        println!(
            "handle_onionized_message > external_global_identity: {:?}",
            external_global_identity
        );

        let body_encrypted_msg = ShinkaiMessageHandler::encrypt_body_if_needed(
            msg.clone(),
            self.encryption_secret_key.clone(),
            external_global_identity.node_encryption_public_key, // other node's encryption public key
        );

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&self.identity_secret_key);
        let msg = ShinkaiMessageHandler::re_sign_message(body_encrypted_msg, signature_sk);

        let recipient_profile_name_string = IdentityManager::extract_recipient_node_global_name(&msg);
        let external_global_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_profile_name_string.clone())
            .await
            .unwrap();

        println!(
            "handle_onionized_message > recipient_profile_name_string: {}",
            recipient_profile_name_string
        );

        let mut db_guard = self.db.lock().await;

        let node_addr = external_global_identity.addr.unwrap();

        Node::send(
            &msg,
            clone_static_secret_key(&self.encryption_secret_key),
            (node_addr, recipient_profile_name_string),
            &mut db_guard,
            self.identity_manager.clone(),
        )
        .await?;
        // println!(
        //     "handle_onionized_message who am I: {:?}",
        //     self.node_profile_name
        // );
        // println!(
        //     "Finished successfully> handle_onionized_message msg: {:?}",
        //     msg
        // );
        // println!("\n\n");
        Ok(())
    }

    pub async fn get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        offset: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    ) {
        // check
        let result = match self
            .db
            .lock()
            .await
            .get_last_unread_messages_from_inbox(inbox_name, limit, offset)
        {
            Ok(messages) => messages,
            Err(e) => {
                error!("Failed to get last unread messages from inbox: {}", e);
                return;
            }
        };

        if let Err(e) = res.send(result).await {
            error!("Failed to send last unread messages: {}", e);
        }
    }

    pub async fn get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    ) {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = match self.db.lock().await.get_last_messages_from_inbox(inbox_name, limit) {
            Ok(messages) => messages,
            Err(e) => {
                error!("Failed to get last messages from inbox: {}", e);
                return;
            }
        };

        // Send the retrieved messages back to the requester.
        if let Err(e) = res.send(result).await {
            error!("Failed to send last messages from inbox: {}", e);
        }
    }

    pub async fn send_public_keys(&self, res: Sender<(SignaturePublicKey, EncryptionPublicKey)>) -> Result<(), Error> {
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
        let messages = db.get_last_messages_from_all(limit).unwrap_or_else(|_| vec![]);
        let _ = res.send(messages).await.map_err(|_| ());
        Ok(())
    }

    pub async fn create_and_send_registration_code(
        &self,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.db.lock().await;
        let code = db.generate_registration_new_code().unwrap_or_else(|_| "".to_string());
        let _ = res.send(code).await.map_err(|_| ());
        Ok(())
    }

    pub async fn mark_as_read_up_to(&self, inbox_name: String, up_to_time: String, res: Sender<String>) {
        // Attempt to mark messages as read in the database
        let result = match self.db.lock().await.mark_as_read_up_to(inbox_name, up_to_time) {
            Ok(()) => "Successfully marked messages as read.".to_string(),
            Err(e) => {
                let error_message = format!("Failed to mark messages as read: {}", e);
                error!("{}", &error_message);
                error_message
            }
        };

        // Send the result back to the requester
        if let Err(e) = res.send(result).await {
            error!("Failed to send result: {}", e);
        }
    }

    pub async fn add_inbox_permission(
        &self,
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    ) {
        let identity = self
            .identity_manager
            .lock()
            .await
            .search_identity(&identity)
            .await
            .unwrap()
            .clone();
        let perm = InboxPermission::from_str(&perm_type).unwrap();
        let result = match self.db.lock().await.add_permission(&inbox_name, &identity, perm) {
            Ok(_) => "Success".to_string(),
            Err(e) => e.to_string(),
        };

        let _ = res.send(result);
    }

    pub async fn remove_inbox_permission(
        &self,
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    ) {
        let identity = self
            .identity_manager
            .lock()
            .await
            .search_identity(&identity)
            .await
            .unwrap()
            .clone();
        // let perm = InboxPermission::from_str(&perm_type).unwrap();
        // First, check if permission exists and remove it if it does
        match self.db.lock().await.remove_permission(&inbox_name, &identity.clone()) {
            Ok(()) => {
                res.send(format!("Permission removed successfully from identity {}.", identity))
                    .await;
            }
            Err(e) => {
                res.send(format!("Error removing permission: {:?}", e)).await;
            }
        }
    }

    pub async fn has_inbox_permission(
        &self,
        inbox_name: String,
        perm_type: String,
        identity_name: String,
        res: Sender<bool>,
    ) {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = self.identity_manager.lock().await;
        let db = self.db.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(&identity_name).await;
        if identity.is_none() {
            // You may want to log an error here or handle it differently
            res.send(false).await;
            return;
        }
        let identity = identity.unwrap();

        // Convert the permission type string to an InboxPermission
        let perm = match InboxPermission::from_str(&perm_type) {
            Ok(perm) => perm,
            Err(_) => {
                // You may want to log an error here or handle it differently
                res.send(false).await;
                return;
            }
        };

        // Call the has_permission function and send the result back
        match db.has_permission(&inbox_name, &identity, perm) {
            Ok(result) => {
                res.send(result).await;
            }
            Err(e) => {
                // Log the error or handle it differently
                // For now, I'll just send false if there's an error
                res.send(false).await;
            }
        }
    }

    pub async fn create_new_job(&self, shinkai_message: ShinkaiMessage, res: Sender<(String, String)>) {
        match self
            .job_manager
            .lock()
            .await
            .process_job_message(shinkai_message, None)
            .await
        {
            Ok(job_id) => {
                // If everything went well, send the job_id back with an empty string for error
                res.send((job_id, String::new())).await;
            }
            Err(err) => {
                // If there was an error, send the error message
                let _ = res.try_send((String::new(), format!("{}", err)));
            }
        };
    }

    pub async fn job_message(&self, job_id: String, shinkai_message: ShinkaiMessage, res: Sender<(String, String)>) {
        // TODO: maybe I don't need job_id?
        match self
            .job_manager
            .lock()
            .await
            .process_job_message(shinkai_message, Some(job_id))
            .await
        {
            Ok(job_id) => {
                // If everything went well, send the job_id back with an empty string for error
                res.send((job_id, String::new())).await;
            }
            Err(err) => {
                // If there was an error, send the error message
                let _ = res.try_send((String::new(), format!("{}", err)));
            }
        };
    }

    pub async fn handle_registration_code_usage(
        &self,
        msg: ShinkaiMessage,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("handle_registration_code_usage");
        let sender_encryption_pk_string = msg.external_metadata.clone().unwrap().other;
        let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).unwrap();

        // Decrypt the message
        let message_to_decrypt = msg.clone();
        let sender_encryption_pk_string = encryption_public_key_to_string(sender_encryption_pk);
        let encryption_secret_key_string = encryption_secret_key_to_string(self.encryption_secret_key.clone());

        let decrypted_content = decrypt_body_message(
            &message_to_decrypt.clone(),
            &self.encryption_secret_key,
            &sender_encryption_pk,
        );

        // println!("handle_registration_code_usage> decrypted_content: {:?}", decrypted_content);

        // You'll need to handle the case where decryption fails
        let decrypted_message = match decrypted_content {
            Ok(message) => message,
            Err(_) => {
                // TODO: add more debug info
                println!("Failed to decrypt the message");
                return Ok(());
            }
        };

        // Deserialize body.content into RegistrationCode
        let content = decrypted_message.clone().body.unwrap().content;
        let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();

        // Extract values from the ShinkaiMessage
        let code = registration_code.code;
        let profile_name = registration_code.profile_name;
        let identity_pk = registration_code.identity_pk;
        let encryption_pk = registration_code.encryption_pk;
        let permission_type = registration_code.permission_type;

        println!("permission_type: {}", permission_type);

        let db = self.db.lock().await;
        let result = db
            .use_registration_code(&code, &profile_name, &identity_pk, &encryption_pk, &permission_type)
            .map_err(|e| e.to_string())
            .map(|_| "true".to_string());
        std::mem::drop(db);

        // TODO: add code if you are the first one some special stuff happens.
        // definition of a shared symmetric encryption key
        // probably we need to sign a message with the pk from the first user
        match result {
            Ok(success) => {
                let signature_pk_obj = string_to_signature_public_key(identity_pk.as_str()).unwrap();
                let encryption_pk_obj = string_to_encryption_public_key(encryption_pk.as_str()).unwrap();
                let permission_type = IdentityType::to_enum(&permission_type).unwrap();
                let full_identity_name = format!("{}/{}", self.node_profile_name.clone(), profile_name.clone());

                let subidentity = Identity {
                    full_identity_name: full_identity_name,
                    addr: None,
                    subidentity_signature_public_key: Some(signature_pk_obj),
                    subidentity_encryption_public_key: Some(encryption_pk_obj),
                    node_encryption_public_key: self.encryption_public_key.clone(),
                    node_signature_public_key: self.identity_public_key.clone(),
                    permission_type: permission_type,
                };
                let mut subidentity_manager = self.identity_manager.lock().await;
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
        res: Sender<Vec<Identity>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let subidentity_manager = self.identity_manager.lock().await;
        let subidentities = subidentity_manager.get_all_subidentities();
        let _ = res.send(subidentities).await.map_err(|_| ());
        Ok(())
    }

    pub async fn ping_all(&self) -> io::Result<()> {
        info!("{} > Pinging all peers {} ", self.listen_address, self.peers.len());
        let mut db_lock = self.db.lock().await;
        for (peer, _) in self.peers.clone() {
            let sender = self.node_profile_name.clone();
            let receiver_profile_identity = self
                .identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&peer.1.clone())
                .await
                .unwrap();
            let receiver = receiver_profile_identity.node_identity_name().to_string();
            let receiver_public_key = receiver_profile_identity.node_encryption_public_key;

            // Important: the receiver doesn't really matter per se as long as it's valid because we are testing the connection
            ping_pong(
                peer,
                PingPong::Ping,
                clone_static_secret_key(&self.encryption_secret_key),
                clone_signature_secret_key(&self.identity_secret_key),
                receiver_public_key,
                sender,
                receiver,
                &mut db_lock,
                self.identity_manager.clone(),
            )
            .await?;
        }
        Ok(())
    }
}
