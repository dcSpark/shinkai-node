use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
};
use super::shinkai_message_error::ShinkaiMessageError;
use super::shinkai_message_schemas::MessageSchemaType;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::Signer;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::info;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl ShinkaiMessage {
    pub fn encrypt_outer_layer(
        &self,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        message_clone.body = MessageBody::encrypt(&message_clone.body, self_sk, destination_pk)?;
        Ok(message_clone)
    }

    pub fn encrypt_inner_layer(
        &self,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        if let MessageBody::Unencrypted(body) = &mut message_clone.body {
            body.message_data = MessageData::encrypt(&body.message_data, self_sk, destination_pk)?;
        }
        Ok(message_clone)
    }

    pub fn decrypt_outer_layer(
        &self,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        match message_clone.body {
            MessageBody::Encrypted(_) => {
                let decrypted_body = message_clone.body.decrypt(self_sk, sender_pk)?;
                message_clone.body = MessageBody::Unencrypted(decrypted_body);
            }
            _ => (),
        }
        Ok(message_clone)
    }

    pub fn decrypt_inner_layer(
        &self,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        if let MessageBody::Unencrypted(body) = &mut message_clone.body {
            match body.message_data {
                MessageData::Encrypted(_) => {
                    let decrypted_data = body.message_data.decrypt(self_sk, sender_pk)?;
                    body.message_data = MessageData::Unencrypted(decrypted_data);
                }
                _ => (),
            }
        } else {
            Err(ShinkaiMessageError::EncryptionError(
                "Body is encrypted. Can't decrypt inner layer".to_string(),
            ))?;
        }
        Ok(message_clone)
    }
}

impl MessageBody {
    pub fn encrypt(
        &self,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<MessageBody, ShinkaiMessageError> {
        match self {
            MessageBody::Unencrypted(body) => MessageBody::encrypt_message_body(body, self_sk, destination_pk),
            MessageBody::Encrypted(_) => Ok(self.clone()),
        }
    }

    pub fn decrypt(
        &self,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiBody, ShinkaiMessageError> {
        match self {
            MessageBody::Encrypted(encrypted_body) => {
                MessageBody::decrypt_message_body(encrypted_body, self_sk, sender_pk)
            }
            MessageBody::Unencrypted(body) => Ok(body.clone()),
        }
    }

    pub fn encrypt_message_body(
        body: &ShinkaiBody,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<MessageBody, ShinkaiMessageError> {
        let body_bytes = bincode::serialize(body).unwrap();
        println!("Serialized body: {:?}", body_bytes);

        let shared_secret = self_sk.diffie_hellman(destination_pk);
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        let result = hasher.finalize();
        let key = GenericArray::clone_from_slice(&result[..]);
        let cipher = ChaCha20Poly1305::new(&key);

        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce[..]);
        let nonce = GenericArray::from_slice(&nonce);

        let ciphertext = cipher.encrypt(nonce, &body_bytes[..]).expect("encryption failure!");
        println!("Encrypted content: {:?}", ciphertext);

        let nonce_and_ciphertext = [nonce.as_slice(), &ciphertext].concat();

        let encrypted_content = format!("encrypted:{}", hex::encode(&nonce_and_ciphertext));
        eprintln!("### MessageBody Encrypted content: {}", encrypted_content);

        Ok(MessageBody::Encrypted(EncryptedShinkaiBody {
            content: encrypted_content,
        }))
    }

    pub fn decrypt_message_body(
        encrypted_body: &EncryptedShinkaiBody,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiBody, ShinkaiMessageError> {
        println!("Decrypting content (before parts): {}", encrypted_body.content);
        let parts: Vec<&str> = encrypted_body.content.split(':').collect();
        println!("Decrypting content (after parts): {:?}", parts);
        match parts.get(0) {
            Some(&"encrypted") => {
                let content = parts.get(1).unwrap_or(&"");
                let shared_secret = self_sk.diffie_hellman(sender_pk);
                let mut hasher = Sha256::new();
                hasher.update(shared_secret.as_bytes());
                let result = hasher.finalize();
                let key = GenericArray::clone_from_slice(&result[..]);
                let cipher = ChaCha20Poly1305::new(&key);

                let decoded = hex::decode(content)
                    .map_err(|e| ShinkaiMessageError::DecryptionError(format!("Failed to decode hex: {}", e)))?;
                println!("Decoded hex content: {:?}", decoded);
                let (nonce, ciphertext) = decoded.split_at(12);
                let nonce = GenericArray::from_slice(nonce);

                let plaintext_bytes = cipher
                    .decrypt(nonce, ciphertext)
                    .map_err(|_| ShinkaiMessageError::DecryptionError("Decryption failure!".to_string()))?;

                println!("Decrypted content: {:?}", plaintext_bytes);

                let decrypted_body: ShinkaiBody = bincode::deserialize(&plaintext_bytes)
                .map_err(|_| ShinkaiMessageError::DecryptionError("Failed to deserialize body".to_string()))?;

                // let decrypted_value: serde_json::Value = serde_json::from_slice(&plaintext_bytes).map_err(|_| {
                //     ShinkaiMessageError::DeserializationError(
                //         "Could not deserialize decrypted content".to_string(),
                //     )
                // })?;
                
                // let decrypted_body: ShinkaiBody = serde_json::from_value(decrypted_value).map_err(|_| {
                //     ShinkaiMessageError::DeserializationError(
                //         "Could not deserialize ShinkaiBody".to_string(),
                //     )
                // })?;

                println!("Deserialized body: {:?}", decrypted_body);
                Ok(decrypted_body)
            }
            _ => Err(ShinkaiMessageError::DecryptionError("Unexpected variant".to_string())),
        }
    }
}

impl MessageData {
    pub fn encrypt(
        &self,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<MessageData, ShinkaiMessageError> {
        match self {
            MessageData::Unencrypted(data) => MessageData::encrypt_message_data(data, self_sk, destination_pk),
            MessageData::Encrypted(_) => Ok(self.clone()),
        }
    }

    pub fn decrypt(
        &self,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiData, ShinkaiMessageError> {
        match self {
            MessageData::Encrypted(encrypted_data) => {
                MessageData::decrypt_message_data(encrypted_data, self_sk, sender_pk)
            }
            MessageData::Unencrypted(data) => Ok(data.clone()),
        }
    }

    pub fn encrypt_message_data(
        data: &ShinkaiData,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<MessageData, ShinkaiMessageError> {
        let shared_secret = self_sk.diffie_hellman(destination_pk);

        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        let result = hasher.finalize();
        let key = GenericArray::clone_from_slice(&result[..]);
        let cipher = ChaCha20Poly1305::new(&key);

        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce[..]);
        let nonce = GenericArray::from_slice(&nonce);

        let schema_str = data.message_content_schema.to_str();
        let combined_content = format!("{}{}", data.message_raw_content, schema_str);
        let ciphertext = cipher
            .encrypt(nonce, combined_content.as_bytes())
            .expect("encryption failure!");

        let nonce_and_ciphertext = [nonce.as_slice(), &ciphertext].concat();

        let content_len = (data.message_raw_content.len() as u64).to_le_bytes();
        let content_schema_len = (schema_str.len() as u64).to_le_bytes();
        let length_prefixed_nonce_and_ciphertext =
            [&content_len[..], &content_schema_len[..], &nonce_and_ciphertext[..]].concat();

        println!(
            "### MessageData Encrypted content: {}",
            hex::encode(&length_prefixed_nonce_and_ciphertext)
        );
        Ok(MessageData::Encrypted(EncryptedShinkaiData {
            content: format!("encrypted:{}", hex::encode(length_prefixed_nonce_and_ciphertext)),
        }))
    }

    pub fn decrypt_message_data(
        encrypted_data: &EncryptedShinkaiData,
        self_sk: &EncryptionStaticKey,
        sender_pk: &EncryptionPublicKey,
    ) -> Result<ShinkaiData, ShinkaiMessageError> {
        println!("Decrypting content: {}", encrypted_data.content);
        let parts: Vec<&str> = encrypted_data.content.split(':').collect();
        match parts.get(0) {
            Some(&"encrypted") => {
                let content = parts.get(1).unwrap_or(&"");
                let shared_secret = self_sk.diffie_hellman(sender_pk);
                let mut hasher = Sha256::new();
                hasher.update(shared_secret.as_bytes());
                let result = hasher.finalize();
                let key = GenericArray::clone_from_slice(&result[..]);
                let cipher = ChaCha20Poly1305::new(&key);

                let decoded = hex::decode(content)
                    .map_err(|e| ShinkaiMessageError::DecryptionError(format!("Failed to decode hex: {}", e)))?;

                let (content_len_bytes, remainder) = decoded.split_at(8);
                let (_, remainder) = remainder.split_at(8);
                let (nonce, ciphertext) = remainder.split_at(12);

                let content_len =
                    u64::from_le_bytes(content_len_bytes.try_into().map_err(|_| {
                        ShinkaiMessageError::DecryptionError("Failed to parse content length".to_string())
                    })?);

                let nonce = GenericArray::from_slice(nonce);

                let plaintext_bytes = cipher
                    .decrypt(nonce, ciphertext)
                    .map_err(|_| ShinkaiMessageError::DecryptionError("Decryption failure!".to_string()))?;

                let (content_bytes, schema_bytes) = plaintext_bytes.split_at(content_len as usize);

                let content = String::from_utf8(content_bytes.to_vec()).map_err(|_| {
                    ShinkaiMessageError::DecryptionError("Failed to decode decrypted content".to_string())
                })?;
                let schema = String::from_utf8(schema_bytes.to_vec()).map_err(|_| {
                    ShinkaiMessageError::DecryptionError("Failed to decode decrypted content schema".to_string())
                })?;
                let schema = MessageSchemaType::from_str(schema.as_str()).ok_or(
                    ShinkaiMessageError::DecryptionError("Failed to parse schema".to_string()),
                )?;

                Ok(ShinkaiData {
                    message_raw_content: content,
                    message_content_schema: schema,
                })
            }
            _ => Err(ShinkaiMessageError::DecryptionError("Unexpected variant".to_string())),
        }
    }
}

impl ShinkaiData {
    pub fn encrypt(
        &self,
        self_sk: &EncryptionStaticKey,
        destination_pk: &EncryptionPublicKey,
    ) -> Result<EncryptedShinkaiData, ShinkaiMessageError> {
        let message_data = MessageData::Unencrypted(self.clone());
        match message_data.encrypt(self_sk, destination_pk)? {
            MessageData::Encrypted(encrypted_data) => Ok(encrypted_data),
            _ => Err(ShinkaiMessageError::EncryptionError("Encryption failed".to_string())),
        }
    }
}
