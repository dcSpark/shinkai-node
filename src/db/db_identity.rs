use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::agent_serialization::SerializedAgent;
use crate::schemas::identity::{StandardIdentity, IdentityType};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rand::RngCore;
use rocksdb::{Error, Options};
use serde_json::to_vec;
use shinkai_message_wasm::shinkai_utils::encryption::{string_to_encryption_public_key, encryption_public_key_to_string, encryption_public_key_to_string_ref};
use shinkai_message_wasm::shinkai_utils::signatures::{string_to_signature_public_key, signature_public_key_to_string, signature_public_key_to_string_ref};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

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

impl ShinkaiDB {
    pub fn generate_registration_new_code(&self) -> Result<String, Error> {
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = bs58::encode(random_bytes).into_string();

        let cf = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();
        self.db.put_cf(cf, &new_code, b"unused")?;

        Ok(new_code)
    }

    pub fn use_registration_code(
        &self,
        registration_code: &str,
        identity_public_key: &str,
        encryption_public_key: &str,
        profile_name: &str,
        permission_type: &str,
        // TODO: extend with toolkit access permissions
        // TODO: extend profiles access from
    ) -> Result<(), ShinkaiDBError> {
        // Check if the code exists in Topic::OneTimeRegistrationCodes and its value is unused
        let cf_codes = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();
        match self.db.get_cf(cf_codes, registration_code)? {
            Some(value) => {
                if RegistrationCodeStatus::from_slice(&value) != RegistrationCodeStatus::Unused {
                    return Err(ShinkaiDBError::CodeAlreadyUsed);
                }
            }
            None => return Err(ShinkaiDBError::CodeNonExistent),
        }

        // Check that the profile name doesn't exist in ProfilesIdentityKey and ProfilesEncryptionKey
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        if self.db.get_cf(cf_identity, profile_name)?.is_some() {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        if self.db.get_cf(cf_encryption, profile_name)?.is_some() {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        let cf_permission = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();
        if self.db.get_cf(cf_permission, profile_name)?.is_some() {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Mark the registration code as used
        batch.put_cf(cf_codes, registration_code, RegistrationCodeStatus::Used.as_bytes());

        // Write to ProfilesIdentityKey and ProfilesEncryptionKey
        batch.put_cf(cf_identity, profile_name, identity_public_key.as_bytes());
        batch.put_cf(cf_encryption, profile_name, encryption_public_key.as_bytes());

        // Write to ProfilesIdentityKey, ProfilesEncryptionKey, and ProfilesIdentityType
        batch.put_cf(cf_permission, profile_name, permission_type.as_bytes());

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_encryption_public_key(&self, identity_public_key: &str) -> Result<String, ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();

        // Get the associated profile name for the identity public key
        let profile_name = match self.db.get_cf(cf_identity, identity_public_key)? {
            Some(name_bytes) => Ok(String::from_utf8_lossy(&name_bytes).to_string()),
            None => Err(ShinkaiDBError::ProfileNameNonExistent),
        }?;

        // Get the associated encryption public key for the profile name
        match self.db.get_cf(cf_encryption, &profile_name)? {
            Some(encryption_key_bytes) => Ok(String::from_utf8_lossy(&encryption_key_bytes).to_string()),
            None => Err(ShinkaiDBError::EncryptionKeyNonExistent),
        }
    }

    pub fn load_all_sub_identities(
        &self,
        my_node_identity_name: String, // TODO: move this to the initializer of the db
    ) -> Result<Vec<StandardIdentity>, ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_permission = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

        // Handle node related information
        let cf_node_encryption = self.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
        let cf_node_identity = self.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();

        let node_encryption_public_key = match self.db.get_cf(cf_node_encryption, &my_node_identity_name)? {
            Some(value) => {
                let key_string = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                string_to_encryption_public_key(&key_string).map_err(|_| ShinkaiDBError::PublicKeyParseError)?
            }
            None => return Err(ShinkaiDBError::ProfileNameNonExistent),
        };

        let node_signature_public_key = match self.db.get_cf(cf_node_identity, &my_node_identity_name)? {
            Some(value) => {
                let key_string = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                string_to_signature_public_key(&key_string).map_err(|_| ShinkaiDBError::PublicKeyParseError)?
            }
            None => return Err(ShinkaiDBError::ProfileNameNonExistent),
        };

        let mut result = Vec::new();
        let iter = self.db.iterator_cf(cf_identity, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, value)) => {
                    let full_identity_name = String::from_utf8(key.to_vec()).unwrap();

                    let subidentity_signature_public_key =
                        string_to_signature_public_key(&String::from_utf8(value.to_vec()).unwrap())
                            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

                    // get the associated encryption public key
                    match self.db.get_cf(cf_encryption, &full_identity_name)? {
                        Some(value) => {
                            let key_string =
                                String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                            let subidentity_encryption_public_key = string_to_encryption_public_key(&key_string)
                                .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

                            match self.db.get_cf(cf_permission, &full_identity_name)? {
                                Some(value) => {
                                    let permission_type_str = String::from_utf8(value.to_vec()).unwrap();
                                    let permission_type = IdentityType::to_enum(&permission_type_str)
                                        .ok_or(ShinkaiDBError::InvalidIdentityType)?;

                                    let identity = StandardIdentity::new(
                                        full_identity_name,
                                        None,
                                        node_encryption_public_key.clone(),
                                        node_signature_public_key.clone(),
                                        Some(subidentity_encryption_public_key),
                                        Some(subidentity_signature_public_key),
                                        permission_type,
                                    );

                                    result.push(identity);
                                }
                                None => return Err(ShinkaiDBError::ProfileNameNonExistent),
                            }
                        }
                        None => return Err(ShinkaiDBError::ProfileNameNonExistent),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    pub fn update_local_node_keys(
        &self,
        my_node_identity_name: String,
        encryption_pk: EncryptionPublicKey,
        signature_pk: SignaturePublicKey,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_encryption = self.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
        let cf_node_identity = self.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();

        let mut batch = rocksdb::WriteBatch::default();

        // Convert public keys to bs58 encoded strings
        let encryption_pk_string = encryption_public_key_to_string(encryption_pk);
        let signature_pk_string = signature_public_key_to_string(signature_pk);

        batch.put_cf(
            cf_node_encryption,
            &my_node_identity_name,
            encryption_pk_string.as_bytes(),
        );
        batch.put_cf(cf_node_identity, &my_node_identity_name, signature_pk_string.as_bytes());

        self.db.write(batch)?;

        Ok(())
    }

    pub fn insert_sub_identity(&self, identity: StandardIdentity) -> Result<(), ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_permission = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

        // Check that the full identity name doesn't exist in the columns
        if self.db.get_cf(cf_identity, &identity.full_identity_name)?.is_some()
            || self.db.get_cf(cf_encryption, &identity.full_identity_name)?.is_some()
            || self.db.get_cf(cf_permission, &identity.full_identity_name)?.is_some()
        {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Convert the encryption and signature public keys to strings
        let sub_identity_public_key = identity
            .subidentity_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());
        let sub_encryption_public_key = identity
            .subidentity_encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());

        // Put the identity details into the columns
        batch.put_cf(
            cf_identity,
            &identity.full_identity_name,
            sub_identity_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_encryption,
            &identity.full_identity_name,
            sub_encryption_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_permission,
            &identity.full_identity_name,
            identity.permission_type.to_string().as_bytes(),
        );

        // TODO: if identity is agent type then also add
        // - Permissions specifying which toolkits/which storage buckets the agent has access to
        // TODO:
        // - Permissions which sub identity has the ability to message the agent

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_subidentity(&self, name: &str) -> Result<(), ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_permission = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

        // Check that the profile name exists in ProfilesIdentityKey, ProfilesEncryptionKey and ProfilesIdentityType
        if self.db.get_cf(cf_identity, name)?.is_none()
            || self.db.get_cf(cf_encryption, name)?.is_none()
            || self.db.get_cf(cf_permission, name)?.is_none()
        {
            return Err(ShinkaiDBError::ProfileNameNonExistent);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Delete from ProfilesIdentityKey, ProfilesEncryptionKey and ProfilesIdentityType
        batch.delete_cf(cf_identity, name);
        batch.delete_cf(cf_encryption, name);
        batch.delete_cf(cf_permission, name);

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }
}
