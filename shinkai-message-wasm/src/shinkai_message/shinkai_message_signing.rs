use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
};
use super::shinkai_message_error::ShinkaiMessageError;
use super::shinkai_message_schemas::MessageSchemaType;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use ed25519_dalek::{Signer, Verifier};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl ShinkaiMessage {
    pub fn sign_outer_layer(&self, secret_key: &SignatureStaticKey) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        if !message_clone.external_metadata.signature.is_empty() {
            message_clone.external_metadata.signature = "".to_string();
        }

        // Convert ShinkaiMessage to bytes
        let message_bytes = bincode::serialize(&message_clone).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(message_bytes);
        let message_hash = hasher.finalize();
        let public_key = SignaturePublicKey::from(secret_key);
        let secret_key_clone = SignatureStaticKey::from_bytes(secret_key.as_ref()).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create SecretKey from bytes: {}", e.to_string()))
        })?;

        let keypair = ed25519_dalek::Keypair {
            public: public_key,
            secret: secret_key_clone,
        };

        let signature = keypair.sign(message_hash.as_slice());
        message_clone.external_metadata.signature = hex::encode(signature.to_bytes());

        Ok(message_clone)
    }

    pub fn verify_outer_layer_signature(
        &self,
        public_key: &ed25519_dalek::PublicKey,
    ) -> Result<bool, ShinkaiMessageError> {
        let base58_signature = &self.external_metadata.signature;

        // Decode the base58 signature to bytes
        let signature_bytes = hex::decode(base58_signature)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e.to_string())))?;

        // Convert the bytes to Signature
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create signature from bytes: {}", e.to_string()))
        })?;

        // Prepare message for hashing - set signature to empty
        let mut message_for_hashing = self.clone();
        message_for_hashing.external_metadata.signature = String::from("");

        // Encode the message to a Vec<u8>
        let bytes = bincode::serialize(&message_for_hashing).unwrap();

        // Create a hash of the message
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let message_hash = hasher.finalize();

        // Verify the signature against the hash of the message
        match public_key.verify(&message_hash.as_slice(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn verify_inner_layer_signature(
        &self,
        public_key: &ed25519_dalek::PublicKey,
    ) -> Result<bool, ShinkaiMessageError> {
        // Convert the MessageBody to a ShinkaiBody
        let shinkai_body = match &self.body {
            MessageBody::Unencrypted(body) => body,
            _ => {
                return Err(ShinkaiMessageError::SigningError(
                    "Message body is not unencrypted".to_string(),
                ))
            }
        };

        // Get the signature from shinkai_body.internal_metadata.signature
        let signature = &shinkai_body.internal_metadata.signature;

        // Decode the base58 signature to bytes
        let signature_bytes = hex::decode(signature)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e.to_string())))?;

        // Convert the bytes to Signature
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create signature from bytes: {}", e.to_string()))
        })?;

        // Convert the MessageBody to a String or &str
        let body_str = match &self.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => &data.message_raw_content,
                _ => {
                    return Err(ShinkaiMessageError::SigningError(
                        "Message data is not unencrypted".to_string(),
                    ))
                }
            },
            _ => {
                return Err(ShinkaiMessageError::SigningError(
                    "Message body is not unencrypted".to_string(),
                ))
            }
        };

        // Hash the body
        let mut hasher = Sha256::new();
        hasher.update(body_str.as_bytes());
        let body_hash = hasher.finalize();

        // Verify the signature against the hash of the body
        match public_key.verify(&body_hash.as_slice(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn calculate_message_hash(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(format!("{:?}", self));
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}
