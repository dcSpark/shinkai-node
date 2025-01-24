use super::shinkai_message::{MessageBody, ShinkaiMessage};
use super::shinkai_message_error::ShinkaiMessageError;

use blake3::Hasher;

use ed25519_dalek::SigningKey;
use ed25519_dalek::{Signer, Verifier};

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::env;
use std::iter::FromIterator;

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
        let sorted_j = Self::to_sorted_json(&j);
        let string = sorted_j.to_string();

        hasher.update(string.as_bytes());
        let result = hasher.finalize();

        hex::encode(result.as_bytes())
    }

    fn to_sorted_json<T: Serialize>(value: &T) -> Value {
        let serialized = serde_json::to_value(value).unwrap();
        Self::sort_json(&serialized)
    }

    fn sort_json(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted_map: BTreeMap<_, _> = map.iter().map(|(k, v)| (k.clone(), Self::sort_json(v))).collect();
                Value::Object(serde_json::Map::from_iter(sorted_map))
            }
            Value::Array(arr) => Value::Array(arr.iter().map(Self::sort_json).collect()),
            _ => value.clone(),
        }
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

        // Sort the JSON Value using the helper function
        let sorted_json = Self::to_sorted_json(&shinkai_body_for_hashing);

        // Convert the sorted JSON Value back to a string
        let shinkai_body_string = sorted_json.to_string();

        Ok(shinkai_body_string)
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
    use crate::{
        shinkai_message::shinkai_message::{EncryptedShinkaiBody, ExternalMetadata, ShinkaiVersion},
        shinkai_utils::{encryption::EncryptionMethod, signatures::unsafe_deterministic_signature_keypair},
    };

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

    #[test]
    fn test_calculate_message_hash_with_empty_outer_signature() {
        // Create a ShinkaiMessage instance with the provided data
        let message = ShinkaiMessage {
            body: MessageBody::Encrypted(EncryptedShinkaiBody {
                content: "encrypted:64b11fe63d2b7c3197f04466522b9b54242d8482fb3d14b1837936920968130433b00abb7f8404eca5b759a5a477658fbf8ccadc6a34895d1c42e446ea113cb2d325ec5b9eeba6c37ba21c55caa13e0d628ab188adf98b08c18768195c9e8c7ff3793992333cb729b3216542a6d1628e06a4b4ce61de62be3a3881bf1e0cf9ecb569c6e5e0672018560fb585496b8ab562efb96a4515e05567550843b251401b0bdce54b847c88ca751e67c20cc59f3a262c951649bfe45d7ab38c76aeba5a9c6dbb009d1726e08735f149b9bdfafa21de0fa429cd57b50d3844192667d307a57a97a9ca7f5b783bdd5e0afbb1d3cd6c3771a250b6791e4021359aef5372c9bc2bbbb2b94ae620412107b3887c7455275fc5cafc32e0290ed713fbde8e5017f77957e0f063e04f36b8d6beebe5d945e362199aadd95c2c530634c0ef124fb058c312d11a7f8d0214270e944d95ab796cdab89a91241fe83128f06bceb5c03f6ee2310142ee748f124b39f2367b8ff13fc52564b4fd4318c268c9a8b899fadf56f9887afe1aa2c99951095b470dd1b17262db6dc6d144307498778c8e900d2ccb8bad0f1589629305352a6f3e496d28eead210b9a3da65be0ab149ec58074f8c5b0fa3201e374de7fc31c207818038d5b897a4a172505475d5a2cb632260bf11af45c1d352d045f2fe63da50aa8d65870bebff890f079348975b57e89a92f493f66121b82f07323789837e0ea3638d69e4af5a19645b28c9ac4362eb4a4c26f84479c942f66dc0eb27c5fe7dbfc45c001ce345d873784547eac26cd25731f4acef76cbe346440c616afe48c7244c5cff1ab7d2af7430f".to_string(),
            }),
            external_metadata: ExternalMetadata {
                sender: "@@my_local_ai.sep-shinkai".to_string(),
                recipient: "@@my_local_ai.sep-shinkai".to_string(),
                scheduled_time: "2024-09-20T04:53:36.773Z".to_string(),
                signature: "".to_string(),
                intra_sender: "main".to_string(),
                other: "".to_string(),
            },
            encryption: EncryptionMethod::DiffieHellmanChaChaPoly1305,
            version: ShinkaiVersion::V1_0,
        };

        // Calculate the hash
        let calculated_hash = message.calculate_message_hash_with_empty_outer_signature();

        // Expected hash
        let expected_hash = "271820710f6653e3aa53b1071d82ebb6073685bce44246c5bad48fa92faa998b";

        // Check that the calculated hash matches the expected hash
        assert_eq!(calculated_hash, expected_hash);
    }
}
