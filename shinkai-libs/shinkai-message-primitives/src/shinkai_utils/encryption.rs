use core::fmt;
use std::error::Error;
// Or use ChaCha20Poly1305Ietf
use blake3::Hasher;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, ToSchema)]
pub enum EncryptionMethod {
    DiffieHellmanChaChaPoly1305,
    None,
}

impl EncryptionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DiffieHellmanChaChaPoly1305 => "default",
            Self::None => "None",
        }
    }

    pub fn from_str(s: &str) -> EncryptionMethod {
        match s {
            "DiffieHellmanChaChaPoly1305" | "default" => EncryptionMethod::DiffieHellmanChaChaPoly1305,
            _ => EncryptionMethod::None,
        }
    }
}

pub fn unsafe_deterministic_encryption_keypair(n: u32) -> (StaticSecret, PublicKey) {
    let mut hasher = Hasher::new();
    hasher.update(&n.to_le_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(hash.as_bytes());

    let secret_key = StaticSecret::from(bytes);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn ephemeral_encryption_keys() -> (StaticSecret, PublicKey) {
    let csprng = OsRng;
    let secret_key = StaticSecret::random_from_rng(csprng);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn encryption_secret_key_to_string(secret_key: StaticSecret) -> String {
    let bytes = secret_key.to_bytes();
    hex::encode(bytes)
}

pub fn encryption_public_key_to_string(public_key: PublicKey) -> String {
    let bytes = public_key.to_bytes();
    hex::encode(bytes)
}

pub fn encryption_public_key_to_string_ref(public_key: &PublicKey) -> String {
    encryption_public_key_to_string(*public_key)
}

pub fn string_to_encryption_static_key(encoded_key: &str) -> Result<StaticSecret, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == 32 {
                let mut array = [0; 32];
                for (i, &byte) in bytes.iter().enumerate() {
                    array[i] = byte;
                }
                Ok(StaticSecret::from(array))
            } else {
                Err("Decoded string length does not match StaticSecret length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn string_to_encryption_public_key(encoded_key: &str) -> Result<PublicKey, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == 32 {
                let mut array = [0; 32];
                for (i, &byte) in bytes.iter().enumerate() {
                    array[i] = byte;
                }
                Ok(PublicKey::from(array))
            } else {
                Err("Decoded string length does not match PublicKey length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn clone_static_secret_key(original: &StaticSecret) -> StaticSecret {
    StaticSecret::from(original.to_bytes())
}

#[derive(Debug)]
pub struct DecryptionError {
    pub details: String,
}

impl DecryptionError {
    pub fn new(msg: &str) -> DecryptionError {
        DecryptionError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for DecryptionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for DecryptionError {
    fn description(&self) -> &str {
        &self.details
    }
}
