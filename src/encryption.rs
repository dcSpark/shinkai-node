use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305; // Or use ChaCha20Poly1305Ietf
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

pub fn encrypt_body_if_needed(
    message: &[u8],
    self_sk: &StaticSecret,
    destination_pk: &PublicKey,
    encryption: Option<&str>,
) -> Option<String> {
    match encryption {
        Some("default") => {
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

            // Here we return the ciphertext (encoded to base64 for easier storage and transmission)
            Some(base64::encode(&ciphertext))
        }
        _ => {
            // Return None if encryption method is not "default"
            None
        }
    }
}
