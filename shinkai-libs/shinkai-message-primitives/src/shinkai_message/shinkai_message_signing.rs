use super::shinkai_message::{MessageBody, ShinkaiMessage};
use super::shinkai_message_error::ShinkaiMessageError;

use blake3::Hasher;

use ed25519_dalek::SigningKey;
use ed25519_dalek::{Signer, Verifier};

use serde_json::json;
use std::convert::TryInto;
use std::env;

impl ShinkaiMessage {
    pub fn sign_outer_layer(&self, secret_key: &SigningKey) -> Result<ShinkaiMessage, ShinkaiMessageError> {
        let mut message_clone = self.clone();

        // Calculate the hash of the message with an empty outer signature
        let message_hash = message_clone.calculate_message_hash_with_empty_outer_signature();

        // Convert the hexadecimal hash back to bytes
        let message_hash_bytes = hex::decode(message_hash)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e)))?;

        let signature = secret_key.sign(message_hash_bytes.as_slice());
        message_clone.external_metadata.signature = hex::encode(signature.to_bytes());

        Ok(message_clone)
    }

    pub fn sign_inner_layer(&mut self, secret_key: &SigningKey) -> Result<(), ShinkaiMessageError> {
        // Calculate the hash of the ShinkaiBody with an empty inner signature
        let shinkai_body_hash = self.calculate_message_hash_with_empty_inner_signature()?;

        // Convert the hexadecimal hash back to bytes
        let shinkai_body_hash_bytes = hex::decode(shinkai_body_hash)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e)))?;

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

    #[allow(dead_code)]
    pub fn verify_outer_layer_signature(
        &self,
        public_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<bool, ShinkaiMessageError> {
        let hex_signature = &self.external_metadata.signature;

        // Decode the base58 signature to bytes
        let signature_bytes = hex::decode(hex_signature)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e)))?;

        // Convert the bytes to Signature
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] =
            signature_bytes_slice
                .try_into()
                .map_err(|e: std::array::TryFromSliceError| {
                    ShinkaiMessageError::SigningError(format!("Failed to convert signature bytes to array: {}", e))
                })?;

        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);

        // Calculate the hash of the message with an empty outer signature
        let message_hash = self.calculate_message_hash_with_empty_outer_signature();

        // Convert the hexadecimal hash back to bytes
        let message_hash_bytes = hex::decode(message_hash)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e)))?;

        // Verify the signature against the hash of the message
        match public_key.verify(&message_hash_bytes, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    #[allow(dead_code)]
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
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode signature: {}", e)))?;

        // Convert the bytes to Signature
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] =
            signature_bytes_slice
                .try_into()
                .map_err(|e: std::array::TryFromSliceError| {
                    ShinkaiMessageError::SigningError(format!("Failed to convert signature bytes to array: {}", e))
                })?;
        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);

        // Calculate the hash of the ShinkaiBody with an empty inner signature
        let shinkai_body_hash = self.calculate_message_hash_with_empty_inner_signature()?;

        // Convert the hexadecimal hash back to bytes
        let shinkai_body_hash_bytes = hex::decode(shinkai_body_hash)
            .map_err(|e| ShinkaiMessageError::SigningError(format!("Failed to decode message hash: {}", e)))?;

        // Verify the signature against the hash of the ShinkaiBody
        match public_key.verify(&shinkai_body_hash_bytes, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    #[allow(dead_code)]
    pub fn calculate_message_hash_for_pagination(&self) -> String {
        let temp_message = match self.clone().update_node_api_data(None) {
            Ok(updated_message) => updated_message,
            Err(_) => self.clone(), // In case of an error, use the original self
        };

        let mut hasher = Hasher::new();
        let j = serde_json::to_string(&temp_message).unwrap_or_default();

        hasher.update(j.as_bytes());
        let result = hasher.finalize();

        hex::encode(result.as_bytes())
    }

    pub fn calculate_message_hash_with_empty_outer_signature(&self) -> String {
        let mut message_clone = match self.clone().update_node_api_data(None) {
            Ok(updated_message) => updated_message,
            Err(_) => self.clone(), // In case of an error, use the original self
        };
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
        shinkai_body_for_hashing.internal_metadata.node_api_data = None;

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

    #[allow(dead_code)]
    pub fn generate_desktop_installation_proof(
        public_key: &ed25519_dalek::VerifyingKey,
        secret_key: &SigningKey,
    ) -> Result<(String, String), ShinkaiMessageError> {
        // Read the secret desktop key from the environment
        let secret_desktop_key = env::var("SECRET_DESKTOP_KEY").map_err(|e| {
            ShinkaiMessageError::SigningError(format!("Failed to read SECRET_DESKTOP_KEY from environment: {}", e))
        })?;

        // Convert the public key to hex
        let public_key_hex = hex::encode(public_key.to_bytes());

        // Combine the public key hex and the secret desktop key
        let combined = format!("{}{}", public_key_hex, secret_desktop_key);

        // Hash the combined value and take the last 4 characters
        let mut hasher = Hasher::new();
        hasher.update(combined.as_bytes());
        let hash_result = hasher.finalize();
        let hash_str = hex::encode(hash_result.as_bytes());
        let last_8_chars = &hash_str[hash_str.len() - 8..];

        // Concatenate the public key hex with the last 4 characters using :::
        let concatenated = format!("{}:::{}", public_key_hex, last_8_chars);

        // Hash the concatenated string
        let mut hasher = Hasher::new();
        hasher.update(concatenated.as_bytes());
        let final_hash_result = hasher.finalize();
        let final_hash_bytes = final_hash_result.as_bytes();

        // Sign the final hash
        let signature = secret_key.sign(final_hash_bytes);

        // Return the signature as a hexadecimal string and the concatenated string
        Ok((hex::encode(signature.to_bytes()), concatenated))
    }
}

#[cfg(test)]
mod tests {
    use crate::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;

    use super::*;
    use ed25519_dalek::Signature;
    use std::env;

    #[test]
    fn test_generate_desktop_installation_proof() {
        // Set the SECRET_DESKTOP_KEY environment variable
        env::set_var("SECRET_DESKTOP_KEY", "HOLA");

        // Generate a deterministic keypair for testing
        let (secret_key, public_key) = unsafe_deterministic_signature_keypair(42);

        // Generate the desktop installation proof
        let result = ShinkaiMessage::generate_desktop_installation_proof(&public_key, &secret_key);
        eprintln!("result: {:?}", result);

        // Check that the result is Ok
        assert!(result.is_ok());

        // Extract the signature and concatenated string
        let (signature_hex, concatenated) = result.unwrap();

        // Verify the concatenated string format
        let public_key_hex = hex::encode(public_key.to_bytes());
        let combined = format!("{}{}", public_key_hex, "HOLA");

        // Hash the combined value and take the last 4 characters
        let mut hasher = Hasher::new();
        hasher.update(combined.as_bytes());
        let hash_result = hasher.finalize();
        let hash_str = hex::encode(hash_result.as_bytes());
        let last_8_chars = &hash_str[hash_str.len() - 8..];

        // Expected concatenated string
        let expected_concatenated = format!("{}:::{}", public_key_hex, last_8_chars);
        assert_eq!(concatenated, expected_concatenated);

        // Hash the concatenated string
        let mut hasher = Hasher::new();
        hasher.update(concatenated.as_bytes());
        let final_hash_result = hasher.finalize();
        let final_hash_bytes = final_hash_result.as_bytes();

        // Verify the signature
        let signature_bytes = hex::decode(signature_hex).expect("Failed to decode hex signature");
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .expect("Failed to convert signature to array");
        let signature = Signature::from_bytes(&signature_array);

        assert!(public_key.verify(final_hash_bytes, &signature).is_ok());
    }
}
