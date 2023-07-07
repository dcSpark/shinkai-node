use std::env;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use crate::shinkai_message::{encryption::{
    string_to_encryption_static_key, ephemeral_encryption_keys
}, signatures::{string_to_signature_secret_key, ephemeral_signature_keypair}};

pub struct NodeKeys {
    pub identity_secret_key: SignatureStaticKey,
    pub identity_public_key: SignaturePublicKey,
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
}

pub fn generate_or_load_keys() -> NodeKeys {
    let (identity_secret_key, identity_public_key) = match env::var("IDENTITY_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_signature_secret_key(&secret_key_str).unwrap();
            let public_key = SignaturePublicKey::from(&secret_key);
            (secret_key, public_key)
        }
        _ => ephemeral_signature_keypair(),
    };

    let (encryption_secret_key, encryption_public_key) = match env::var("ENCRYPTION_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_encryption_static_key(&secret_key_str).unwrap();
            let public_key = x25519_dalek::PublicKey::from(&secret_key);
            (secret_key, public_key)
        }
        _ => ephemeral_encryption_keys(),
    };

    NodeKeys {
        identity_secret_key,
        identity_public_key,
        encryption_secret_key,
        encryption_public_key,
    }
}
