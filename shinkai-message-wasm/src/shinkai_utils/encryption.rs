use core::fmt;
use std::convert::TryInto;
use std::error::Error;

use bs58::decode;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use js_sys::Uint8Array;
// Or use ChaCha20Poly1305Ietf
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use x25519_dalek::{PublicKey, StaticSecret};

use super::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
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

#[wasm_bindgen]
pub fn convert_encryption_sk_string_to_encryption_pk_string(encryption_sk: String) -> Result<String, JsValue> {
    let my_encryption_sk_type = string_to_encryption_static_key(&encryption_sk)?;
    let my_encryption_pk = x25519_dalek::PublicKey::from(&my_encryption_sk_type);
    Ok(encryption_public_key_to_string(my_encryption_pk))
}

pub fn encryption_secret_key_to_jsvalue(secret: &StaticSecret) -> JsValue {
    let bytes = secret.to_bytes().to_vec();
    JsValue::from(Uint8Array::from(&bytes[..]))
}

pub fn encryption_public_key_to_jsvalue(public_key: &PublicKey) -> JsValue {
    let bytes = public_key.as_bytes().to_vec();
    JsValue::from(Uint8Array::from(&bytes[..]))
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

pub fn encrypt_body(
    body: &[u8],
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
            let ciphertext = cipher.encrypt(nonce, body).expect("encryption failure!");

            // Here we return the nonce and ciphertext (encoded to bs58 for easier storage and transmission)
            let nonce_and_ciphertext = [nonce.as_slice(), &ciphertext].concat();

            Some(bs58::encode(&nonce_and_ciphertext).into_string())
        }
        EncryptionMethod::None => None,
    }
}

pub fn encrypt_string_content(
    content: String,
    content_schema: String,
    self_sk: &StaticSecret,
    destination_pk: &PublicKey,
    encryption: &str,
) -> Option<String> {
    match EncryptionMethod::from_str(encryption) {
        EncryptionMethod::DiffieHellmanChaChaPoly1305 => {
            let shared_secret = self_sk.diffie_hellman(destination_pk);

            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            let result = hasher.finalize();
            let key = GenericArray::clone_from_slice(&result[..]);
            let cipher = ChaCha20Poly1305::new(&key);

            let mut nonce = [0u8; 12];
            OsRng.fill_bytes(&mut nonce[..]);
            let nonce = GenericArray::from_slice(&nonce);

            // Combine the content and content_schema into a single string
            let combined_content = format!("{}{}", content, content_schema);
            let ciphertext = cipher
                .encrypt(nonce, combined_content.as_bytes())
                .expect("encryption failure!");

            let nonce_and_ciphertext = [nonce.as_slice(), &ciphertext].concat();

            // Prepend the length of the content and content_schema as 8-byte strings
            // the maximum value of an 8-byte integer in megabytes would be (2^64 - 1) * 2^-17
            // = 140,737,488,355.328 MB (or roughly 140 terabytes).
            let content_len = (content.len() as u64).to_le_bytes();
            let content_schema_len = (content_schema.len() as u64).to_le_bytes();
            let length_prefixed_nonce_and_ciphertext = [
                &content_len[..],
                &content_schema_len[..],
                &nonce_and_ciphertext[..],
            ]
            .concat();

            Some(bs58::encode(length_prefixed_nonce_and_ciphertext).into_string())
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

pub fn decrypt_body_message(
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
            let decrypted_body = ShinkaiMessageHandler::decode_body(plaintext_bytes.as_slice().to_vec());

            decrypted_message.body = Some(decrypted_body);
        }
        EncryptionMethod::None => (),
    }

    Ok(decrypted_message)
}

pub fn decrypt_content_message(
    encrypted_content: String,
    encryption: &str,
    self_sk: &StaticSecret,
    sender_pk: &PublicKey,
) -> Result<(String, String), DecryptionError> {
    match EncryptionMethod::from_str(encryption) {
        EncryptionMethod::DiffieHellmanChaChaPoly1305 => {
            let shared_secret = self_sk.diffie_hellman(sender_pk);
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            let result = hasher.finalize();
            let key = GenericArray::clone_from_slice(&result[..]);
            let cipher = ChaCha20Poly1305::new(&key);

            let decoded = bs58::decode(&encrypted_content)
                .into_vec()
                .expect("Failed to decode bs58");

            let (content_len_bytes, remainder) = decoded.split_at(8);
            let (_, remainder) = remainder.split_at(8);
            let (nonce, ciphertext) = remainder.split_at(12);

            let content_len = u64::from_le_bytes(
                content_len_bytes
                    .try_into()
                    .map_err(|_| DecryptionError::new("Failed to parse content length"))?,
            );

            let nonce = GenericArray::from_slice(nonce);

            let plaintext_bytes = cipher
                .decrypt(nonce, ciphertext)
                .expect("Decryption failure!");

            let (content_bytes, schema_bytes) = plaintext_bytes.split_at(content_len as usize);

            let content = String::from_utf8(content_bytes.to_vec())
                .expect("Failed to decode decrypted content");
            let schema = String::from_utf8(schema_bytes.to_vec())
                .expect("Failed to decode decrypted content schema");

            Ok((content, schema))
        }
        EncryptionMethod::None => Err(DecryptionError::new("Encryption method is None")),
    }
}