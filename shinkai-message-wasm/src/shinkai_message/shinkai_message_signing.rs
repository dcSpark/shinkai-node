use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
};
use super::shinkai_message_error::ShinkaiMessageError;
use super::shinkai_message_schemas::MessageSchemaType;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use ed25519_dalek::Signer;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
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

    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(format!("{:?}", self));
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

