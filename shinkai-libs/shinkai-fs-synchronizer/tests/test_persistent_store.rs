#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use shinkai_file_synchronizer::communication::generate_encryption_keys;
    use shinkai_file_synchronizer::communication::generate_signature_keys;
    use shinkai_file_synchronizer::persistent::Storage;
    use shinkai_file_synchronizer::persistent::StorageData;
    use tempfile::tempdir;
    use x25519_dalek::StaticSecret;

    #[test]
    fn test_write_and_read_data() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_storage.json");
        let storage = Storage::new(
            dir.path().to_str().unwrap().to_string(),
            "test_storage.json".to_string(),
        );

        let data = StorageData {
            encryption_secret_key: "encryption_key".to_string(),
            signature_secret_key: "signature_key".to_string(),
            receiver_public_key: "receiver_key".to_string(),
        };

        storage.write_data(&data).unwrap();
        let read_data = storage.read_data().unwrap();

        assert_eq!(data.encryption_secret_key, read_data.encryption_secret_key);
        assert_eq!(data.signature_secret_key, read_data.signature_secret_key);
        assert_eq!(data.receiver_public_key, read_data.receiver_public_key);
    }

    #[tokio::test]
    async fn test_write_and_read_keys() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_str().unwrap().to_string(), "test_keys.json".to_string());

        let (my_device_encryption_sk, my_device_encryption_pk) = generate_encryption_keys().await;
        let (my_device_signature_sk, my_device_signing_key) = generate_signature_keys().await;

        let (profile_encryption_sk, profile_encryption_pk) = generate_encryption_keys().await;
        let (profile_signature_sk, profile_signing_key) = generate_signature_keys().await;

        let encryption_secret_key = my_device_encryption_sk.clone();
        let signature_secret_key = my_device_signing_key.clone();

        // this one is irrelevant here, because it will be overwritten by the encryption_publick_key from the node
        let receiver_public_key = profile_encryption_pk.clone();

        storage.write_encryption_secret_key(&encryption_secret_key).unwrap();
        storage.write_signature_secret_key(&signature_secret_key).unwrap();
        storage.write_receiver_public_key(&receiver_public_key).unwrap();

        let read_encryption_secret_key = storage.read_encryption_secret_key();
        let read_signature_secret_key = storage.read_signature_secret_key();
        let read_receiver_public_key = storage.read_receiver_public_key();

        assert_eq!(encryption_secret_key.to_bytes(), read_encryption_secret_key.to_bytes());
        assert_eq!(signature_secret_key.to_bytes(), read_signature_secret_key.to_bytes());
        assert_eq!(receiver_public_key.as_bytes(), read_receiver_public_key.as_bytes());
    }
}
