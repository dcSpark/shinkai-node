/*
The x25519_dalek crate is specifically designed for Diffie-Hellman key agreement and does not include functionality for creating digital signatures. The types in this crate (PublicKey and StaticSecret) don't have methods for signing and verifying messages, which is the functionality you're looking for.

Digital signatures usually require a different kind of key pair than the one used for Diffie-Hellman. In the case of Curve25519, which the x25519_dalek crate is based on, the related digital signature algorithm is Ed25519, which is implemented in the ed25519_dalek crate.

So, you would indeed need to use a different crate (such as ed25519_dalek) to create and verify digital signatures if you stick with the dalek-cryptography ecosystem.

 */

use blake3::Hasher;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::convert::TryInto;

pub fn unsafe_deterministic_signature_keypair(n: u32) -> (SigningKey, VerifyingKey) {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&n.to_le_bytes());
    let hash = hasher.finalize();

    let secret_key = SigningKey::from_bytes(hash.as_bytes());
    let public_key = VerifyingKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn ephemeral_signature_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

pub fn clone_signature_secret_key(original: &SigningKey) -> SigningKey {
    original.clone()
}

pub fn signature_secret_key_to_string(secret_key: SigningKey) -> String {
    let bytes = secret_key.as_bytes();
    hex::encode(bytes)
}

pub fn signature_public_key_to_string(public_key: VerifyingKey) -> String {
    let bytes = public_key.as_bytes();
    hex::encode(bytes)
}

pub fn signature_public_key_to_string_ref(public_key: &VerifyingKey) -> String {
    signature_public_key_to_string(*public_key)
}

pub fn string_to_signature_secret_key(encoded_key: &str) -> Result<SigningKey, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == ed25519_dalek::SECRET_KEY_LENGTH {
                let bytes_array: [u8; 32] = bytes.try_into().unwrap();
                Ok(SigningKey::from_bytes(&bytes_array))
            } else {
                Err("Decoded string length does not match SecretKey length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn string_to_signature_public_key(encoded_key: &str) -> Result<VerifyingKey, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == ed25519_dalek::PUBLIC_KEY_LENGTH {
                let bytes_array: [u8; 32] = bytes.try_into().unwrap();
                VerifyingKey::from_bytes(&bytes_array).map_err(|_| "Failed to create PublicKey from bytes")
            } else {
                Err("Decoded string length does not match PublicKey length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn hash_signature_public_key(public_key: &VerifyingKey) -> String {
    let mut hasher = Hasher::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash.as_bytes())
}
