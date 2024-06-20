use blake3::Hasher;
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::KeyInit;

use rand::RngCore;
use hex;

pub fn random_aes_encryption_key() -> [u8; 32] {
    let mut symmetrical = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut symmetrical);
    
    let key = GenericArray::from_slice(&symmetrical);
    let _cipher = Aes256Gcm::new(key);

    symmetrical
}

pub fn unsafe_deterministic_aes_encryption_key(n: u32) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&n.to_le_bytes());
    let hash = hasher.finalize();

    let mut symmetrical = [0u8; 32];
    symmetrical.copy_from_slice(hash.as_bytes());

    let key = GenericArray::from_slice(&symmetrical);
    let _cipher = Aes256Gcm::new(key);

    symmetrical
}

pub fn aes_encryption_key_to_string(key: [u8; 32]) -> String {
    hex::encode(key)
}

pub fn hash_of_aes_encryption_key_hex(key: [u8; 32]) -> String {
    let key_hex = aes_encryption_key_to_string(key);
    let mut hasher = Hasher::new();
    hasher.update(key_hex.as_bytes());
    let result = hasher.finalize();
    hex::encode(result.as_bytes())
}

pub fn aes_nonce_to_hex_string(nonce: &[u8]) -> String {
    hex::encode(nonce)
}

pub fn hex_string_to_aes_nonce(hex_string: &str) -> Result<[u8; 12], hex::FromHexError> {
    let bytes = hex::decode(hex_string)?;
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes);
    Ok(nonce)
}

pub fn calculate_blake3_hash(input: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result.as_bytes())
}