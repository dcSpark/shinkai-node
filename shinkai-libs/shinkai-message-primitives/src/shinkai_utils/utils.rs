use blake3::Hasher;
use rand::RngCore;

pub fn hash_string(input: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result.as_bytes())
}

pub fn random_string() -> String {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);

    let mut hasher = Hasher::new();
    hasher.update(&key);
    let hash = hasher.finalize();

    hex::encode(hash.as_bytes())
}