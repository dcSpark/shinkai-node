/*
The x25519_dalek crate is specifically designed for Diffie-Hellman key agreement and does not include functionality for creating digital signatures. The types in this crate (PublicKey and StaticSecret) don't have methods for signing and verifying messages, which is the functionality you're looking for.

Digital signatures usually require a different kind of key pair than the one used for Diffie-Hellman. In the case of Curve25519, which the x25519_dalek crate is based on, the related digital signature algorithm is Ed25519, which is implemented in the ed25519_dalek crate.

So, you would indeed need to use a different crate (such as ed25519_dalek) to create and verify digital signatures if you stick with the dalek-cryptography ecosystem.

 */

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use blake3::{Hasher};

use crate::shinkai_message::shinkai_message::MessageBody;
use crate::shinkai_message::shinkai_message::MessageData;
use crate::shinkai_message::shinkai_message::ShinkaiMessage;

pub fn unsafe_deterministic_signature_keypair(n: u32) -> (SecretKey, PublicKey) {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&n.to_le_bytes());
    let hash = hasher.finalize();

    let secret_key = SecretKey::from_bytes(hash.as_bytes()).expect("Failed to create SecretKey from hash");
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn ephemeral_signature_keypair() -> (SecretKey, PublicKey) {
    #[warn(deprecated)]
    let mut csprng = rand_os::OsRng::new().unwrap();
    let keypair = Keypair::generate(&mut csprng);
    (keypair.secret, keypair.public)
}

pub fn clone_signature_secret_key(original: &SecretKey) -> SecretKey {
    SecretKey::from_bytes(&original.to_bytes()).unwrap()
}

pub fn signature_secret_key_to_string(secret_key: SecretKey) -> String {
    let bytes = secret_key.as_bytes();
    hex::encode(bytes)
}

pub fn signature_public_key_to_string(public_key: PublicKey) -> String {
    let bytes = public_key.as_bytes();
    hex::encode(bytes)
}

pub fn signature_public_key_to_string_ref(public_key: &PublicKey) -> String {
    signature_public_key_to_string(public_key.clone())
}

pub fn string_to_signature_secret_key(encoded_key: &str) -> Result<SecretKey, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == ed25519_dalek::SECRET_KEY_LENGTH {
                SecretKey::from_bytes(&bytes).map_err(|_| "Failed to create SecretKey from bytes")
            } else {
                Err("Decoded string length does not match SecretKey length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn string_to_signature_public_key(encoded_key: &str) -> Result<PublicKey, &'static str> {
    match hex::decode(encoded_key) {
        Ok(bytes) => {
            if bytes.len() == ed25519_dalek::PUBLIC_KEY_LENGTH {
                PublicKey::from_bytes(&bytes).map_err(|_| "Failed to create PublicKey from bytes")
            } else {
                Err("Decoded string length does not match PublicKey length")
            }
        }
        Err(_) => Err("Failed to decode hex string"),
    }
}

pub fn hash_signature_public_key(public_key: &PublicKey) -> String {
    let mut hasher = Hasher::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash.as_bytes())
}
