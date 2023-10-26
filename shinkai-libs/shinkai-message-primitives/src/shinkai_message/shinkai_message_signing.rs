use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
};
use super::shinkai_message_error::ShinkaiMessageError;
use super::shinkai_message_schemas::MessageSchemaType;
use blake3::Hasher;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use ed25519_dalek::{Signer, Verifier};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl ShinkaiMessage {
    pub fn sign_outer_layer(&self, secret_key: &SignatureStaticKey) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();
        if !message_clone.external_metadata.signature.is_empty() {
            message_clone.external_metadata.signature = "".to_string();
        }

        // Calculate the hash of the message with an empty outer signature
        let message_hash = message_clone.calculate_message_hash_with_empty_outer_signature();

        // Convert the hexadecimal hash back to bytes
        let message_hash_bytes = hex::decode(&message_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;

        let public_key = SignaturePublicKey::from(secret_key);
        let secret_key_clone = SignatureStaticKey::from_bytes(secret_key.as_ref()).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create SecretKey from bytes: {}", e.to_string()))
        })?;

        let keypair = ed25519_dalek::Keypair {
            public: public_key,
            secret: secret_key_clone,
        };

        let signature = keypair.sign(message_hash_bytes.as_slice());
        message_clone.external_metadata.signature = hex::encode(signature.to_bytes());

        Ok(message_clone)
    }

    pub fn sign_inner_layer(&mut self, secret_key: &SignatureStaticKey) -> Result<(), ShinkaiMessageError> {
        // Calculate the hash of the ShinkaiBody with an empty inner signature
        let shinkai_body_hash = self.calculate_message_hash_with_empty_inner_signature()?;

        // Convert the hexadecimal hash back to bytes
        let shinkai_body_hash_bytes = hex::decode(&shinkai_body_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;

        // Create a keypair from the secret key
        let public_key = SignaturePublicKey::from(secret_key);
        let secret_key_clone = SignatureStaticKey::from_bytes(secret_key.as_ref()).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create SecretKey from bytes: {}", e.to_string()))
        })?;
        let keypair = ed25519_dalek::Keypair {
            public: public_key,
            secret: secret_key_clone,
        };

        // Sign the hash of the ShinkaiBody
        let signature = keypair.sign(shinkai_body_hash_bytes.as_slice());

        // Store the signature in the internal metadata
        match &mut self.body {
            MessageBody::Unencrypted(body) => {
                body.internal_metadata.signature = hex::encode(signature.to_bytes());
            }
            _ => {
                return Err(ShinkaiMessageError::SigningError(
                    "Message body is not unencrypted".to_string(),
                ))
            }
        };

        Ok(())
    }

    pub fn verify_outer_layer_signature(
        &self,
        public_key: &ed25519_dalek::PublicKey,
    ) -> Result<bool, ShinkaiMessageError> {
        let hex_signature = &self.external_metadata.signature;

        // Decode the base58 signature to bytes
        let signature_bytes = hex::decode(hex_signature)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e.to_string())))?;

        // Convert the bytes to Signature
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to create signature from bytes: {}", e.to_string()))
        })?;

        // Calculate the hash of the message with an empty outer signature
        let message_hash = self.calculate_message_hash_with_empty_outer_signature();

        // Convert the hexadecimal hash back to bytes
        let message_hash_bytes = hex::decode(&message_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;

        // Verify the signature against the hash of the message
        match public_key.verify(&message_hash_bytes, &signature) {
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

        // Prepare ShinkaiBody for hashing - set signature to empty
        let mut shinkai_body_for_hashing = shinkai_body.clone();
        shinkai_body_for_hashing.internal_metadata.signature = String::from("");

        // Convert the ShinkaiBody to bytes
        let shinkai_body_bytes = bincode::serialize(&shinkai_body_for_hashing).unwrap();

        // Hash the ShinkaiBody
        let mut hasher = Sha256::new();
        hasher.update(shinkai_body_bytes);
        let shinkai_body_hash = hasher.finalize();

        // Verify the signature against the hash of the ShinkaiBody
        match public_key.verify(&shinkai_body_hash.as_slice(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn calculate_message_hash(&self) -> String {
        let mut hasher = Hasher::new();

        let j = json!(self);
        let string = j.to_string();

        hasher.update(string.as_bytes());
        let result = hasher.finalize();

        hex::encode(result.as_bytes())
    }

    pub fn calculate_message_hash_with_empty_outer_signature(&self) -> String {
        let mut message_clone = self.clone();
        message_clone.external_metadata.signature = "".to_string();

        let mut hasher = Hasher::new();
        let j = json!(message_clone);
        let string = j.to_string();

        hasher.update(string.as_bytes());
        let result = hasher.finalize();

        hex::encode(result.as_bytes())
    }

    pub fn calculate_message_hash_with_empty_inner_signature(&self) -> Result<String, ShinkaiMessageError> {
        // Check if the body is unencrypted
        let shinkai_body = match &self.body {
            MessageBody::Unencrypted(body) => body,
            _ => {
                return Err(ShinkaiMessageError::SigningError(
                    "Message body is not unencrypted".to_string(),
                ))
            }
        };

        // Prepare ShinkaiBody for hashing - set signature to empty
        let mut shinkai_body_for_hashing = shinkai_body.clone();
        shinkai_body_for_hashing.internal_metadata.signature = String::from("");

        // Convert the ShinkaiBody to bytes
        let shinkai_body_bytes = bincode::serialize(&shinkai_body_for_hashing).unwrap();

        // Hash the ShinkaiBody
        let mut hasher = Hasher::new();
        hasher.update(&shinkai_body_bytes);
        let shinkai_body_hash = hasher.finalize();

        Ok(hex::encode(shinkai_body_hash.as_bytes()))
    }
}
