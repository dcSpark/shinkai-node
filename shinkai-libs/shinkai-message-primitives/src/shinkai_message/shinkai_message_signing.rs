use crate::shinkai_utils::signatures::signature_public_key_to_string;

use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
};
use super::shinkai_message_error::ShinkaiMessageError;
use super::shinkai_message_schemas::MessageSchemaType;
use blake3::Hasher;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::{Signer, Verifier};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::json;
use std::convert::TryInto;

impl ShinkaiMessage {
    pub fn sign_outer_layer(&self, secret_key: &SigningKey) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();

        // Calculate the hash of the message with an empty outer signature
        let message_hash = message_clone.calculate_message_hash_with_empty_outer_signature();

        // Convert the hexadecimal hash back to bytes
        let message_hash_bytes = hex::decode(&message_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;

        let verifying_key = secret_key.verifying_key();
        let signature = secret_key.sign(message_hash_bytes.as_slice());
        message_clone.external_metadata.signature = hex::encode(signature.to_bytes());

        Ok(message_clone)
    }

    pub fn sign_inner_layer(&mut self, secret_key: &SigningKey) -> Result<(), ShinkaiMessageError> {
        // Calculate the hash of the ShinkaiBody with an empty inner signature
        let shinkai_body_hash = self.calculate_message_hash_with_empty_inner_signature()?;

        // Convert the hexadecimal hash back to bytes
        let shinkai_body_hash_bytes = hex::decode(&shinkai_body_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;

        // Sign the hash of the ShinkaiBody
        let signature = secret_key.sign(shinkai_body_hash_bytes.as_slice());

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
        public_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<bool, ShinkaiMessageError> {
        let hex_signature = &self.external_metadata.signature;
    
        // Decode the base58 signature to bytes
        let signature_bytes = hex::decode(hex_signature)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e.to_string())))?;
    
        // Convert the bytes to Signature
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] = signature_bytes_slice.try_into().map_err(|e: std::array::TryFromSliceError| {
            ShinkaiMessageError::SigningError(format!("Failed to convert signature bytes to array: {}", e.to_string()))
        })?;
        
        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);
    
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
        public_key: &ed25519_dalek::VerifyingKey,
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
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] = signature_bytes_slice.try_into().map_err(|e: std::array::TryFromSliceError| {
            ShinkaiMessageError::SigningError(format!("Failed to convert signature bytes to array: {}", e.to_string()))
        })?;
        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);
    
        // Calculate the hash of the ShinkaiBody with an empty inner signature
        let shinkai_body_hash = self.calculate_message_hash_with_empty_inner_signature()?;
    
        // Convert the hexadecimal hash back to bytes
        let shinkai_body_hash_bytes = hex::decode(&shinkai_body_hash).map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e.to_string()))
        })?;
    
        // Verify the signature against the hash of the ShinkaiBody
        match public_key.verify(&shinkai_body_hash_bytes, &signature) {
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
        // Get the ShinkaiBody ready for hashing
        let shinkai_body_string = self.inner_content_ready_for_hashing()?;

        // Hash the ShinkaiBody
        let mut hasher = Hasher::new();
        hasher.update(shinkai_body_string.as_bytes());
        let shinkai_body_hash = hasher.finalize();

        Ok(hex::encode(shinkai_body_hash.as_bytes()))
    }

    pub fn inner_content_ready_for_hashing(&self) -> Result<String, ShinkaiMessageError> {
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

        // Convert the ShinkaiBody to a JSON Value
        let mut shinkai_body_value: serde_json::Value = serde_json::to_value(&shinkai_body_for_hashing).unwrap();

        // Sort the JSON Value
        ShinkaiMessage::sort_json_value(&mut shinkai_body_value);

        // Convert the sorted JSON Value back to a string
        let shinkai_body_string = shinkai_body_value.to_string();

        Ok(shinkai_body_string)
    }

    // Function to sort a JSON Value
    fn sort_json_value(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                let mut sorted = serde_json::Map::new();
                let keys: Vec<_> = map.keys().cloned().collect();
                for k in keys {
                    let v = map.remove(&k).unwrap();
                    sorted.insert(k, v);
                }
                *map = sorted;
                for (_, v) in map.iter_mut() {
                    Self::sort_json_value(v);
                }
            }
            serde_json::Value::Array(vec) => {
                for v in vec {
                    Self::sort_json_value(v);
                }
            }
            _ => {}
        }
    }
}
