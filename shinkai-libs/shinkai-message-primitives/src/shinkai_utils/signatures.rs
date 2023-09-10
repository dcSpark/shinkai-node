/*
The x25519_dalek crate is specifically designed for Diffie-Hellman key agreement and does not include functionality for creating digital signatures. The types in this crate (PublicKey and StaticSecret) don't have methods for signing and verifying messages, which is the functionality you're looking for.

Digital signatures usually require a different kind of key pair than the one used for Diffie-Hellman. In the case of Curve25519, which the x25519_dalek crate is based on, the related digital signature algorithm is Ed25519, which is implemented in the ed25519_dalek crate.

So, you would indeed need to use a different crate (such as ed25519_dalek) to create and verify digital signatures if you stick with the dalek-cryptography ecosystem.

 */

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use sha2::{Digest, Sha256};

use crate::shinkai_message::shinkai_message::MessageBody;
use crate::shinkai_message::shinkai_message::MessageData;
use crate::shinkai_message::shinkai_message::ShinkaiMessage;

pub fn unsafe_deterministic_signature_keypair(n: u32) -> (SecretKey, PublicKey) {
    let mut hasher = Sha256::new();
    hasher.update(n.to_le_bytes());
    let hash = hasher.finalize();

    let secret_key = SecretKey::from_bytes(&hash.as_slice()).expect("Failed to create SecretKey from hash");
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
    let mut hasher = Sha256::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

pub fn sign_message(secret_key: &SecretKey, message: ShinkaiMessage) -> String {
    let mut message_clone = message.clone();
    if !message_clone.external_metadata.signature.is_empty() {
        message_clone.external_metadata.signature = "".to_string();
    }

    // Convert ShinkaiMessage to bytes
    let message_bytes = bincode::serialize(&message_clone).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(message_bytes);
    let message_hash = hasher.finalize();
    let public_key = PublicKey::from(secret_key);
    let secret_key_clone = SecretKey::from_bytes(secret_key.as_ref()).expect("Failed to create SecretKey from bytes");

    let keypair = ed25519_dalek::Keypair {
        public: public_key,
        secret: secret_key_clone,
    };

    let signature = keypair.sign(message_hash.as_slice());
    hex::encode(signature.to_bytes())
}

pub fn sign_message_with_body(secret_key: &SecretKey, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    let body_hash = hasher.finalize();
    let public_key = PublicKey::from(secret_key);
    let secret_key_clone = SecretKey::from_bytes(secret_key.as_ref()).expect("Failed to create SecretKey from bytes");

    let keypair = ed25519_dalek::Keypair {
        public: public_key,
        secret: secret_key_clone,
    };

    let signature = keypair.sign(body_hash.as_slice());
    hex::encode(signature.to_bytes())
}

pub fn sign_message_body(secret_key: &SecretKey, message: ShinkaiMessage) -> String {
    // TODO: needs to sign content and the other stuff inside internal_metadata
    let body = serde_json::to_string(&message.body).expect("Failed to serialize message body");
    sign_message_with_body(secret_key, &body)
}

pub fn verify_signature(
    public_key: &ed25519_dalek::PublicKey,
    message: &ShinkaiMessage,
) -> Result<bool, Box<dyn std::error::Error>> {
    let base58_signature = &message.external_metadata.signature;

    // Decode the base58 signature to bytes
    let signature_bytes = hex::decode(base58_signature)?;

    // Convert the bytes to Signature
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes)?;

    // Prepare message for hashing - set signature to empty
    let mut message_for_hashing = message.clone();
    message_for_hashing.external_metadata.signature = String::from("");

    // Encode the message to a Vec<u8>
    let bytes = bincode::serialize(&message_for_hashing).unwrap();

    // Create a hash of the message
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let message_hash = hasher.finalize();

    // Verify the signature against the hash of the message
    Ok(public_key.verify(&message_hash.as_slice(), &signature).is_ok())
}

pub fn verify_signature_with_body(
    public_key: &ed25519_dalek::PublicKey,
    body: &str,
    signature: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Decode the base58 signature to bytes
    let signature_bytes = hex::decode(signature)?;

    // Convert the bytes to Signature
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes)?;

    // Hash the body
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    let body_hash = hasher.finalize();

    // Verify the signature against the hash of the body
    Ok(public_key.verify(&body_hash.as_slice(), &signature).is_ok())
}

pub fn verify_signature_body(
    public_key: &ed25519_dalek::PublicKey,
    message: &ShinkaiMessage,
) -> Result<bool, Box<dyn std::error::Error>> {
    let body = serde_json::to_string(&message.body)?;
    let signature = message.external_metadata.signature.clone();
    verify_signature_with_body(public_key, &body, &signature)
}