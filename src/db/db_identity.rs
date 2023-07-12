use rand::RngCore;
use crate::shinkai_message::encryption::encryption_public_key_to_string_ref;
use crate::shinkai_message::signatures::signature_public_key_to_string_ref;
use crate::shinkai_message::{encryption::string_to_encryption_public_key, signatures::string_to_signature_public_key};
use crate::network::subidentities::Subidentity;
use super::{db_errors::ShinkaiMessageDBError, db::{Topic}, ShinkaiMessageDB};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rocksdb::{Error};

#[derive(PartialEq)]
pub enum RegistrationCodeStatus {
    Unused,
    Used,
}

impl RegistrationCodeStatus {
    pub fn from_slice(slice: &[u8]) -> Self {
        match slice {
            b"unused" => Self::Unused,
            _ => Self::Used,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Unused => b"unused",
            Self::Used => b"used",
        }
    }
}

impl ShinkaiMessageDB {
    pub fn generate_registration_new_code(&self) -> Result<String, Error> {
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = bs58::encode(random_bytes).into_string();

        let cf = self
            .db
            .cf_handle(Topic::OneTimeRegistrationCodes.as_str())
            .unwrap();
        self.db.put_cf(cf, &new_code, b"unused")?;

        Ok(new_code)
    }

    pub fn use_registration_code(
        &self,
        registration_code: &str,
        identity_public_key: &str,
        encryption_public_key: &str,
        profile_name: &str,
    ) -> Result<(), ShinkaiMessageDBError> {
        // Check if the code exists in Topic::OneTimeRegistrationCodes and its value is unused
        let cf_codes = self
            .db
            .cf_handle(Topic::OneTimeRegistrationCodes.as_str())
            .unwrap();
        match self.db.get_cf(cf_codes, registration_code)? {
            Some(value) => {
                if RegistrationCodeStatus::from_slice(&value) != RegistrationCodeStatus::Unused {
                    return Err(ShinkaiMessageDBError::CodeAlreadyUsed);
                }
            }
            None => return Err(ShinkaiMessageDBError::CodeNonExistent),
        }

        // Check that the profile name doesn't exist in ProfilesIdentityKey and ProfilesEncryptionKey
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .unwrap();
        if self.db.get_cf(cf_identity, profile_name)?.is_some() {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }

        let cf_encryption = self
            .db
            .cf_handle(Topic::ProfilesEncryptionKey.as_str())
            .unwrap();
        if self.db.get_cf(cf_encryption, profile_name)?.is_some() {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Mark the registration code as used
        batch.put_cf(
            cf_codes,
            registration_code,
            RegistrationCodeStatus::Used.as_bytes(),
        );

        // Write to ProfilesIdentityKey and ProfilesEncryptionKey
        batch.put_cf(cf_identity, profile_name, identity_public_key.as_bytes());
        batch.put_cf(
            cf_encryption,
            profile_name,
            encryption_public_key.as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_encryption_public_key(
        &self,
        identity_public_key: &str,
    ) -> Result<String, ShinkaiMessageDBError> {
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .unwrap();
        let cf_encryption = self
            .db
            .cf_handle(Topic::ProfilesEncryptionKey.as_str())
            .unwrap();

        // Get the associated profile name for the identity public key
        let profile_name = match self.db.get_cf(cf_identity, identity_public_key)? {
            Some(name_bytes) => Ok(String::from_utf8_lossy(&name_bytes).to_string()),
            None => Err(ShinkaiMessageDBError::ProfileNameNonExistent),
        }?;

        // Get the associated encryption public key for the profile name
        match self.db.get_cf(cf_encryption, &profile_name)? {
            Some(encryption_key_bytes) => {
                Ok(String::from_utf8_lossy(&encryption_key_bytes).to_string())
            }
            None => Err(ShinkaiMessageDBError::EncryptionKeyNonExistent),
        }
    }

    pub fn load_all_sub_identities(
        &self,
    ) -> Result<Vec<(String, EncryptionPublicKey, SignaturePublicKey)>, ShinkaiMessageDBError> {
        let cf_encryption = self
            .db
            .cf_handle(Topic::ProfilesEncryptionKey.as_str())
            .unwrap();
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .unwrap();

        let mut result = Vec::new();

        let iter = self
            .db
            .iterator_cf(cf_encryption, rocksdb::IteratorMode::Start);
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, value)) => {
                    let name = String::from_utf8(key.to_vec()).unwrap();
                    let encryption_public_key = string_to_encryption_public_key(
                        &String::from_utf8(value.to_vec()).unwrap(),
                    )
                    .map_err(|_| ShinkaiMessageDBError::PublicKeyParseError)?;

                    // get the associated signature public key
                    match self.db.get_cf(cf_identity, &name)? {
                        Some(value) => {
                            let signature_public_key = string_to_signature_public_key(
                                &String::from_utf8(value.to_vec()).unwrap(),
                            )
                            .map_err(|_| ShinkaiMessageDBError::PublicKeyParseError)?;
                            result.push((name, encryption_public_key, signature_public_key));
                        }
                        None => return Err(ShinkaiMessageDBError::ProfileNameNonExistent),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    pub fn remove_identity(&self, name: &str) -> Result<(), ShinkaiMessageDBError> {
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .unwrap();
        let cf_encryption = self
            .db
            .cf_handle(Topic::ProfilesEncryptionKey.as_str())
            .unwrap();

        // Check that the profile name exists in ProfilesIdentityKey and ProfilesEncryptionKey
        if self.db.get_cf(cf_identity, name)?.is_none()
            || self.db.get_cf(cf_encryption, name)?.is_none()
        {
            return Err(ShinkaiMessageDBError::ProfileNameNonExistent);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Delete from ProfilesIdentityKey and ProfilesEncryptionKey
        batch.delete_cf(cf_identity, name);
        batch.delete_cf(cf_encryption, name);

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn insert_sub_identity(&self, identity: Subidentity) -> Result<(), ShinkaiMessageDBError> {
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .unwrap();
        let cf_encryption = self
            .db
            .cf_handle(Topic::ProfilesEncryptionKey.as_str())
            .unwrap();

        // Check that the profile name doesn't exist in ProfilesIdentityKey and ProfilesEncryptionKey
        if self.db.get_cf(cf_identity, &identity.name)?.is_some()
            || self.db.get_cf(cf_encryption, &identity.name)?.is_some()
        {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Write to ProfilesIdentityKey and ProfilesEncryptionKey
        let identity_public_key = identity
            .signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());

        let encryption_public_key = identity
            .encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());

        batch.put_cf(
            cf_identity,
            &identity.name,
            identity_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_encryption,
            &identity.name,
            encryption_public_key.as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }
}