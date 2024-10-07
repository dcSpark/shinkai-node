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

/// Counts the number of tokens from a single message string for llama3 model,
/// where every three normal letters (a-zA-Z) allow an empty space to not be counted,
/// and other symbols are counted as 1 token.
/// This implementation avoids floating point arithmetic by scaling counts.
pub fn count_tokens_from_message_llama3(message: &str) -> u64 {
    let mut token_count = 0;
    let mut alphabetic_count = 0; // Total count of alphabetic characters
    let mut space_count = 0; // Total count of spaces
                             // ^ need to fix this

    // First pass: count alphabetic characters and spaces
    for c in message.chars() {
        if c.is_ascii_alphabetic() {
            alphabetic_count += 1;
        } else if c.is_whitespace() {
            space_count += 1;
        }
    }

    // Calculate how many spaces can be ignored
    let spaces_to_ignore = alphabetic_count / 3;

    // Determine the alphabetic token weight based on the number of alphabetic characters
    let alphabetic_token_weight = if alphabetic_count > 500 { 8 } else { 10 };

    // Second pass: count tokens, adjusting for spaces that can be ignored
    for c in message.chars() {
        if c.is_ascii_alphabetic() {
            token_count += alphabetic_token_weight; // Counting as 1/3, so add 1 to the scaled count
        } else if c.is_whitespace() {
            if spaces_to_ignore > 0 {
                space_count -= 10; // Reduce the count of spaces to ignore by the scaling factor
            } else {
                token_count += 30; // Count the space as a full token if not enough alphabetic characters
            }
        } else {
            token_count += 30; // Non-alphabetic characters count as a full token, add 3 to the scaled count
        }
    }

    (token_count / 30) + 1 // Divide the scaled count by 30 and floor the result, add 1 to account for any remainder
}
