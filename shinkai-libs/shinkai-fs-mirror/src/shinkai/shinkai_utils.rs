use hex::decode;
use super::shinkai_device_keys::ShinkaiDeviceKeys;
use libsodium_sys::*;

pub fn decrypt_exported_keys(encrypted_body: &str, passphrase: &str) -> Result<ShinkaiDeviceKeys, &'static str> {
    unsafe {
        if libsodium_sys::sodium_init() == -1 {
            return Err("Failed to initialize libsodium");
        }

        if !encrypted_body.starts_with("encrypted:") {
            return Err("Unexpected variant");
        }

        let content = &encrypted_body["encrypted:".len()..];
        let salt_hex = &content[..32];
        let nonce_hex = &content[32..56];
        let ciphertext_hex = &content[56..];

        let salt = decode(salt_hex).map_err(|_| "Failed to decode salt")?;
        let nonce = decode(nonce_hex).map_err(|_| "Failed to decode nonce")?;
        let ciphertext = decode(ciphertext_hex).map_err(|_| "Failed to decode ciphertext")?;

        let mut key = vec![0u8; 32];
        let passphrase_cstr = std::ffi::CString::new(passphrase).expect("Passphrase conversion failed");

        let pwhash_result = crypto_pwhash(
            key.as_mut_ptr(),
            key.len() as u64,
            passphrase_cstr.as_ptr(),
            passphrase.len() as u64,
            salt.as_ptr(),
            crypto_pwhash_OPSLIMIT_INTERACTIVE as u64,
            crypto_pwhash_MEMLIMIT_INTERACTIVE as usize,
            crypto_pwhash_ALG_DEFAULT as i32,
        );

        if pwhash_result != 0 {
            return Err("Key derivation failed");
        }

        let mut decrypted_data = vec![0u8; ciphertext.len() - crypto_aead_chacha20poly1305_IETF_ABYTES as usize];
        let mut decrypted_len = 0u64;

        let decryption_result = crypto_aead_chacha20poly1305_ietf_decrypt(
            decrypted_data.as_mut_ptr(),
            &mut decrypted_len,
            std::ptr::null_mut(),
            ciphertext.as_ptr(),
            ciphertext.len() as u64,
            std::ptr::null(),
            0,
            nonce.as_ptr() as *const u8,
            key.as_ptr(),
        );
        if decryption_result != 0 {
            return Err("Decryption failed");
        }

        decrypted_data.truncate(decrypted_len as usize);
        let decrypted_str = String::from_utf8(decrypted_data).map_err(|_| "Failed to decode decrypted data")?;
        eprintln!("Decrypted data: {}", decrypted_str);
        serde_json::from_str(&decrypted_str).map_err(|_| "Failed to parse decrypted data into DeviceKeys")
    }
}