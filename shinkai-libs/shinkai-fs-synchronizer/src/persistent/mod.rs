use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

// TODO: this must be more generic to store whatever we need
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StorageData {
    pub encryption_secret_key: String,
    pub signature_secret_key: String,
    pub receiver_public_key: String,
}

pub struct Storage {
    pub path: String,
    pub file_name: String,
}

impl Storage {
    pub fn new(path: String, file_name: String) -> Self {
        Self { path, file_name }
    }

    pub fn write_data(&self, data: &StorageData) -> io::Result<()> {
        let file_path = Path::new(&self.path).join(&self.file_name);
        let file = File::create(file_path)?;
        serde_json::to_writer(file, data)?;
        Ok(())
    }

    pub fn read_data(&self) -> io::Result<StorageData> {
        let file_path = Path::new(&self.path).join(&self.file_name);
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let data: StorageData = serde_json::from_str(&contents)?;
        Ok(data)
    }

    pub fn write_encryption_secret_key(&self, key: &x25519_dalek::StaticSecret) -> io::Result<()> {
        let mut data = self.read_data().unwrap_or_default();
        data.encryption_secret_key = hex::encode(key.to_bytes());
        let result = self.write_data(&data);
        println!(
            "Encryption secret key saved to: {}",
            Path::new(&self.path).join(&self.file_name).display()
        );
        result
    }

    pub fn write_signature_secret_key(&self, key: &ed25519_dalek::SigningKey) -> io::Result<()> {
        let mut data = self.read_data().unwrap_or_default();
        data.signature_secret_key = hex::encode(key.to_bytes());
        let result = self.write_data(&data);
        println!(
            "Signature secret key saved to: {}",
            Path::new(&self.path).join(&self.file_name).display()
        );
        result
    }

    pub fn write_receiver_public_key(&self, key: &x25519_dalek::PublicKey) -> io::Result<()> {
        let mut data = self.read_data().unwrap_or_default();
        data.receiver_public_key = hex::encode(key.as_bytes());
        let result = self.write_data(&data);
        println!(
            "Receiver public key saved to: {}",
            Path::new(&self.path).join(&self.file_name).display()
        );
        result
    }

    pub fn read_encryption_secret_key(&self) -> x25519_dalek::StaticSecret {
        let data = self.read_data().unwrap();
        let key_bytes: [u8; 32] = hex::decode(data.encryption_secret_key)
            .expect("Decoding failed")
            .try_into()
            .expect("Incorrect length");
        x25519_dalek::StaticSecret::from(key_bytes)
    }

    pub fn read_signature_secret_key(&self) -> ed25519_dalek::SigningKey {
        let data = self.read_data().unwrap();
        let key_bytes = hex::decode(data.signature_secret_key).expect("Decoding failed");
        let key_array: [u8; 32] = key_bytes.try_into().expect("Incorrect length");
        ed25519_dalek::SigningKey::from_bytes(&key_array)
    }

    pub fn read_receiver_public_key(&self) -> x25519_dalek::PublicKey {
        let data = self.read_data().unwrap();
        let key_bytes = hex::decode(data.receiver_public_key).expect("Decoding failed");
        let key_array: [u8; 32] = key_bytes.try_into().expect("Incorrect length");
        x25519_dalek::PublicKey::from(key_array)
    }
}
