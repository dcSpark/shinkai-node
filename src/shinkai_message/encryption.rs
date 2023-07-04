use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305; // Or use ChaCha20Poly1305Ietf
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

pub enum EncryptionMethod {
    DiffieHellmanChaChaPoly1305,
    None,
}

impl EncryptionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DiffieHellmanChaChaPoly1305 => "DiffieHellmanChaChaPoly1305",
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

pub fn unsafe_deterministic_double_private_key(n: u32) -> ((StaticSecret, PublicKey), (StaticSecret, PublicKey)) {
    let mut hasher = Sha256::new();
    hasher.update(n.to_le_bytes());
    let hash = hasher.finalize();

    let mut bytes1 = [0u8; 32];
    bytes1.copy_from_slice(&hash[0..32]);

    let secret_key1 = StaticSecret::from(bytes1);
    let public_key1 = PublicKey::from(&secret_key1);

    hasher = Sha256::new();
    hasher.update((n + 1_000_000).to_le_bytes());
    let hash = hasher.finalize();

    let mut bytes2 = [0u8; 32];
    bytes2.copy_from_slice(&hash[0..32]);

    let secret_key2 = StaticSecret::from(bytes2);
    let public_key2 = PublicKey::from(&secret_key2);

    ((secret_key1, public_key1), (secret_key2, public_key2))
}


pub fn unsafe_deterministic_single_private_key(n: u32) -> (StaticSecret, PublicKey) {
    let mut hasher = Sha256::new();
    hasher.update(n.to_le_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&hash[0..32]);

    let secret_key = StaticSecret::from(bytes);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn ephemeral_keys() -> (StaticSecret, PublicKey) {
    #[allow(deprecated)]
    let mut csprng = rand_os::OsRng::new().unwrap();
    let secret_key = StaticSecret::new(&mut csprng);
    let public_key = PublicKey::from(&secret_key);
    (secret_key, public_key)
}

pub fn secret_key_to_string(secret_key: StaticSecret) -> String {
    let bytes = secret_key.to_bytes();
    bs58::encode(&bytes).into_string()
}

pub fn public_key_to_string(public_key: PublicKey) -> String {
    let bytes = public_key.to_bytes();
    bs58::encode(&bytes).into_string()
}

pub fn string_to_static_key(encoded_key: &str) -> Result<StaticSecret, &'static str> {
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

pub fn string_to_public_key(encoded_key: &str) -> Result<PublicKey, &'static str> {
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

pub fn hash_public_key(public_key: PublicKey) -> String {
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
    encryption: Option<&str>,
) -> Option<String> {
    match EncryptionMethod::from_str(encryption.unwrap_or("None")) {
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

pub fn decrypt_body_content(
    ciphertext: &[u8],
    self_sk: &StaticSecret,
    sender_pk: &PublicKey,
    encryption: Option<&str>,
) -> Option<String> {
    match EncryptionMethod::from_str(encryption.unwrap_or("None")) {
        EncryptionMethod::DiffieHellmanChaChaPoly1305 => {
            let shared_secret = self_sk.diffie_hellman(&sender_pk);

            // Convert the shared secret into a suitable key
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            let result = hasher.finalize();
            let key = GenericArray::clone_from_slice(&result[..]); // panics if lengths are unequal

            let cipher = ChaCha20Poly1305::new(&key);

            let decoded = bs58::decode(ciphertext).into_vec().expect("Failed to decode bs58");
            let (nonce, ciphertext) = decoded.split_at(12);
            let nonce = GenericArray::from_slice(nonce);

            // Decrypt ciphertext
            let plaintext = cipher
                .decrypt(nonce, ciphertext)
                .expect("decryption failure!");

            // Here we return the plaintext (encoded as a string for easier use)
            Some(String::from_utf8(plaintext).expect("Failed to convert decrypted bytes to String"))
        }
        EncryptionMethod::None => None,
    }
}
