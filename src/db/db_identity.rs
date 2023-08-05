use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::schemas::identity::{
    DeviceIdentity, IdentityPermissions, IdentityType, StandardIdentity, StandardIdentityType,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rocksdb::{Error, Options};
use serde_json::to_vec;
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_public_key_to_string_ref, string_to_encryption_public_key,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_public_key_to_string_ref, string_to_signature_public_key,
};
use warp::path::full;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl ShinkaiDB {
    pub fn get_encryption_public_key(&self, identity_public_key: &str) -> Result<String, ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();

        // Get the associated profile name for the identity public key
        let profile_name = match self.db.get_cf(cf_identity, identity_public_key)? {
            Some(name_bytes) => Ok(String::from_utf8_lossy(&name_bytes).to_string()),
            None => Err(ShinkaiDBError::ProfileNameNonExistent(identity_public_key.to_string())),
        }?;

        // Get the associated encryption public key for the profile name
        match self.db.get_cf(cf_encryption, &profile_name)? {
            Some(encryption_key_bytes) => Ok(String::from_utf8_lossy(&encryption_key_bytes).to_string()),
            None => Err(ShinkaiDBError::EncryptionKeyNonExistent),
        }
    }

    pub fn load_all_sub_identities(
        &self,
        my_node_identity_name: String,
    ) -> Result<Vec<StandardIdentity>, ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_type = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();
        let cf_permission = self.db.cf_handle(Topic::ProfilesPermission.as_str()).unwrap(); // Added this line

        // Handle node related information
        let cf_node_encryption = self.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
        let cf_node_identity = self.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();

        let node_encryption_public_key = match self.db.get_cf(cf_node_encryption, &my_node_identity_name)? {
            Some(value) => {
                let key_string = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                string_to_encryption_public_key(&key_string).map_err(|_| ShinkaiDBError::PublicKeyParseError)?
            }
            None => {
                return Err(ShinkaiDBError::ProfileNameNonExistent(
                    my_node_identity_name.to_string(),
                ))
            }
        };

        let node_signature_public_key = match self.db.get_cf(cf_node_identity, &my_node_identity_name)? {
            Some(value) => {
                let key_string = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                string_to_signature_public_key(&key_string).map_err(|_| ShinkaiDBError::PublicKeyParseError)?
            }
            None => {
                return Err(ShinkaiDBError::ProfileNameNonExistent(
                    my_node_identity_name.to_string(),
                ))
            }
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

                    match self.db.get_cf(cf_encryption, &full_identity_name)? {
                        Some(value) => {
                            let key_string =
                                String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                            let subidentity_encryption_public_key = string_to_encryption_public_key(&key_string)
                                .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

                            match self.db.get_cf(cf_type, &full_identity_name)? {
                                Some(value) => {
                                    let identity_type_str = String::from_utf8(value.to_vec()).unwrap();
                                    let identity_type = StandardIdentityType::to_enum(&identity_type_str).ok_or(
                                        ShinkaiDBError::InvalidIdentityType(format!(
                                            "Invalid identity type for: {}",
                                            identity_type_str
                                        )),
                                    )?;

                                    match self.db.get_cf(cf_permission, &full_identity_name)? {
                                        // Updated this line
                                        Some(value) => {
                                            let permissions_str = String::from_utf8(value.to_vec()).unwrap();
                                            let permissions = IdentityPermissions::from_str(&permissions_str)
                                                .ok_or(ShinkaiDBError::InvalidPermissionsType)?;
                                            let full_identity_name = match ShinkaiName::new(full_identity_name.clone())
                                            {
                                                Ok(name) => name,
                                                Err(_) => {
                                                    return Err(ShinkaiDBError::InvalidIdentityName(
                                                        full_identity_name.clone(),
                                                    ))
                                                }
                                            };

                                            let identity = StandardIdentity::new(
                                                full_identity_name,
                                                None,
                                                node_encryption_public_key.clone(),
                                                node_signature_public_key.clone(),
                                                Some(subidentity_encryption_public_key),
                                                Some(subidentity_signature_public_key),
                                                identity_type,
                                                permissions, // Added this line
                                            );
                                            result.push(identity);
                                        }
                                        None => return Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name)),
                                    }
                                }
                                None => return Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name)),
                            }
                        }
                        None => return Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name)),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    pub fn insert_profile(&self, identity: StandardIdentity) -> Result<(), ShinkaiDBError> {
        println!("identity.full_identity_name: {}", identity.full_identity_name);
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_identity_type = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();
        let cf_permission_type = self.db.cf_handle(Topic::ProfilesPermission.as_str()).unwrap();

        // Check that the full identity name doesn't exist in the columns
        if self.db.get_cf(cf_identity, &identity.full_identity_name)?.is_some()
            || self.db.get_cf(cf_encryption, &identity.full_identity_name)?.is_some()
            || self
                .db
                .get_cf(cf_identity_type, &identity.full_identity_name)?
                .is_some()
        {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Convert the encryption and signature public keys to strings
        let sub_identity_public_key = identity
            .profile_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());
        let sub_encryption_public_key = identity
            .profile_encryption_public_key
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
            cf_identity_type,
            &identity.full_identity_name,
            identity.identity_type.to_string().as_bytes(),
        );

        batch.put_cf(
            cf_permission_type,
            &identity.full_identity_name,
            identity.permission_type.to_string().as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_profile_permission(&self, profile_name: &str) -> Result<IdentityPermissions, ShinkaiDBError> {
        let cf_permission = self.db.cf_handle(Topic::ProfilesPermission.as_str()).unwrap();
        match self.db.get_cf(cf_permission, profile_name)? {
            Some(value) => {
                let permission_str = std::str::from_utf8(&value).map_err(|_| {
                    ShinkaiDBError::InvalidPermissionType(format!("Invalid permission type: {:?}", value))
                })?;
                IdentityPermissions::from_str(permission_str).ok_or(ShinkaiDBError::InvalidPermissionType(format!(
                    "Invalid permission type: {:?}",
                    value
                )))
            }
            None => Err(ShinkaiDBError::PermissionNotFound(format!(
                "No permission found for profile: {}",
                profile_name
            ))),
        }
    }

    pub fn add_device_to_profile(&self, device: DeviceIdentity) -> Result<(), ShinkaiDBError> {
        // Get the profile name from the device identity name
        let profile_name = match device.full_identity_name.get_profile_name() {
            Some(name) => name,
            None => {
                return Err(ShinkaiDBError::InvalidIdentityName(
                    device.full_identity_name.to_string(),
                ))
            }
        };

        // First, make sure that the profile the device is to be linked with exists
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        if self.db.get_cf(cf_identity, profile_name)?.is_none() {
            return Err(ShinkaiDBError::ProfileNotFound);
        }

        // Get a handle to the device column family
        let cf_device = self.db.cf_handle(Topic::Devices.as_str()).unwrap();

        // Check that the full device identity name doesn't already exist in the column
        if self.db.get_cf(cf_device, &device.full_identity_name)?.is_some() {
            return Err(ShinkaiDBError::DeviceIdentityAlreadyExists);
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Convert the public keys to strings
        let device_signature_public_key = device
            .device_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| String::new());

        // Add the device information to the batch
        batch.put_cf(
            cf_device,
            &device.full_identity_name,
            device_signature_public_key.as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_profile(&self, name: &str) -> Result<(), ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_permission = self.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

        // Check that the profile name exists in ProfilesIdentityKey, ProfilesEncryptionKey and ProfilesIdentityType
        if self.db.get_cf(cf_identity, name)?.is_none()
            || self.db.get_cf(cf_encryption, name)?.is_none()
            || self.db.get_cf(cf_permission, name)?.is_none()
        {
            return Err(ShinkaiDBError::ProfileNameNonExistent(name.to_string()));
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

    pub fn get_profile(&self, full_identity_name: ShinkaiName) -> Result<Option<StandardIdentity>, ShinkaiDBError> {
        let profile_name = full_identity_name
            .get_profile_name()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("ProfilesIdentityKey".to_string()))?;
        let cf_encryption =
            self.db
                .cf_handle(Topic::ProfilesEncryptionKey.as_str())
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                    "ProfilesEncryptionKey".to_string(),
                ))?;
        let cf_type = self
            .db
            .cf_handle(Topic::ProfilesIdentityType.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("ProfilesIdentityType".to_string()))?;
        let cf_permission = self
            .db
            .cf_handle(Topic::ProfilesPermission.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("ProfilesPermission".to_string()))?;

        let identity_public_key_bytes = match self.db.get_cf(cf_identity, profile_name.clone())? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let encryption_public_key_bytes = self
            .db
            .get_cf(cf_encryption, profile_name.clone())?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;
        let identity_type_bytes = self
            .db
            .get_cf(cf_type, profile_name.clone())?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;
        let permission_type_bytes = self
            .db
            .get_cf(cf_permission, profile_name.clone())?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;

        let identity_public_key_str =
            String::from_utf8(identity_public_key_bytes.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
        let encryption_public_key_str =
            String::from_utf8(encryption_public_key_bytes.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
        let identity_type_str =
            String::from_utf8(identity_type_bytes.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
        let permission_type_str =
            String::from_utf8(permission_type_bytes.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;

        let identity_public_key = string_to_signature_public_key(&identity_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;
        let encryption_public_key = string_to_encryption_public_key(&encryption_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;
        let identity_type = StandardIdentityType::to_enum(&identity_type_str)
            .ok_or(ShinkaiDBError::InvalidIdentityType(identity_type_str.clone()))?;
        let permission_type =
            IdentityPermissions::from_str(&permission_type_str).ok_or(ShinkaiDBError::InvalidPermissionsType)?;

        let (node_encryption_public_key, node_signature_public_key) = self.get_local_node_keys(&profile_name)?;

        Ok(Some(StandardIdentity {
            full_identity_name,
            addr: None,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key: Some(encryption_public_key),
            profile_signature_public_key: Some(identity_public_key),
            identity_type,
            permission_type,
        }))
    }
}
