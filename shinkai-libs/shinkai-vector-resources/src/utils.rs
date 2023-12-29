use blake3::Hasher;
use rand::RngCore;

/// Hashes a String using Blake3, returning the hash as an output String
pub fn hash_string(input: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result.as_bytes())
}

/// Generates a random hex String
pub fn random_string() -> String {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);

    let mut hasher = Hasher::new();
    hasher.update(&key);
    let hash = hasher.finalize();

    hex::encode(hash.as_bytes())
}
