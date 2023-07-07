use core::fmt;
use std::error::Error;

use bs58::decode;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305; // Or use ChaCha20Poly1305Ietf
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::shinkai_message_proto::{Body, ShinkaiMessage};

use super::shinkai_message_handler::ShinkaiMessageHandler;

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
            "DiffieHellmanChaChaPoly1305" | "default" => {
                EncryptionMethod::DiffieHellmanChaChaPoly1305
            }
            _ => EncryptionMethod::None,
        }
    }
}

pub fn unsafe_deterministic_encryption_keypair(n: u32) -> (StaticSecret, PublicKey) {
    let mut hasher = Sha256::new();
    hasher.update(n.to_le_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&hash[0..32]);

    let secret_key = StaticSecret::from(bytes);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn ephemeral_encryption_keys() -> (StaticSecret, PublicKey) {
    #[allow(deprecated)]
    let mut csprng = rand_os::OsRng::new().unwrap();
    let secret_key = StaticSecret::new(&mut csprng);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn encryption_secret_key_to_string(secret_key: StaticSecret) -> String {
    let bytes = secret_key.to_bytes();
    bs58::encode(&bytes).into_string()
}

pub fn encryption_public_key_to_string(public_key: PublicKey) -> String {
    let bytes = public_key.to_bytes();
    bs58::encode(&bytes).into_string()
}

pub fn encryption_public_key_to_string_ref(public_key: &PublicKey) -> String {
    encryption_public_key_to_string(public_key.clone())
}

pub fn string_to_encryption_static_key(encoded_key: &str) -> Result<StaticSecret, &'static str> {
    println!("encoded_key: {}", encoded_key);
    match bs58::decode(encoded_key).into_vec() {
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
        Err(_) => Err("Failed to decode bs58 string"),
    }
}

pub fn string_to_encryption_public_key(encoded_key: &str) -> Result<PublicKey, &'static str> {
    match bs58::decode(encoded_key).into_vec() {
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
        Err(_) => Err("Failed to decode bs58 string"),
    }
}

pub fn clone_static_secret_key(original: &StaticSecret) -> StaticSecret {
    StaticSecret::from(original.to_bytes())
}

pub fn hash_encryption_public_key(public_key: PublicKey) -> String {
    let bytes = public_key.to_bytes();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("{:x}", result)
}

pub fn encrypt_body_if_needed(
    message: &[u8],
    self_sk: &StaticSecret,
    destination_pk: &PublicKey,
    encryption: &str,
) -> Option<String> {
    match EncryptionMethod::from_str(encryption) {
        EncryptionMethod::DiffieHellmanChaChaPoly1305 => {
            let shared_secret = self_sk.diffie_hellman(&destination_pk);
            
            // Convert the shared secret into a suitable key
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            let result = hasher.finalize();
            let key = GenericArray::clone_from_slice(&result[..]); // panics if lengths are unequal

            let cipher = ChaCha20Poly1305::new(&key);

            // Generate a unique nonce for each operation
            let mut nonce = [0u8; 12];
            OsRng.fill_bytes(&mut nonce[..]);
            let nonce = GenericArray::from_slice(&nonce);

            // Encrypt message
            let ciphertext = cipher.encrypt(nonce, message).expect("encryption failure!");

            // Here we return the nonce and ciphertext (encoded to bs58 for easier storage and transmission)
            let nonce_and_ciphertext = [nonce.as_slice(), &ciphertext].concat();

            Some(bs58::encode(&nonce_and_ciphertext).into_string())
        }
        EncryptionMethod::None => None,
    }
}

#[derive(Debug)]
pub struct DecryptionError {
    pub details: String,
}

impl DecryptionError {
    fn new(msg: &str) -> DecryptionError {
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

pub fn decrypt_message(
    message: &ShinkaiMessage,
    self_sk: &StaticSecret,
    sender_pk: &PublicKey,
) -> Result<ShinkaiMessage, DecryptionError> {
    let mut decrypted_message = message.clone();

    match EncryptionMethod::from_str(message.encryption.as_str()) {
        EncryptionMethod::DiffieHellmanChaChaPoly1305 => {
            let shared_secret = self_sk.diffie_hellman(&sender_pk);

            // Convert the shared secret into a suitable key
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            let result = hasher.finalize();
            let key = GenericArray::clone_from_slice(&result[..]); // panics if lengths are unequal

            let cipher = ChaCha20Poly1305::new(&key);

            let decoded = bs58::decode(&message.body.as_ref().unwrap().content)
                .into_vec()
                .map_err(|_| DecryptionError::new("Failed to decode bs58"))?;
            let (nonce, ciphertext) = decoded.split_at(12);
            let nonce = GenericArray::from_slice(nonce);
            
            // Decrypt ciphertext
            let plaintext_bytes = cipher
                .decrypt(nonce, ciphertext)
                .map_err(|_| DecryptionError::new("Decryption failure!"))?;

            // Convert the decrypted bytes back into a Body
            let decrypted_body =
                ShinkaiMessageHandler::decode_body(plaintext_bytes.as_slice().to_vec())
                    .expect("Failed to decode decrypted body");

            decrypted_message.body = Some(decrypted_body);
        }
        EncryptionMethod::None => (),
    }

    Ok(decrypted_message)
}
