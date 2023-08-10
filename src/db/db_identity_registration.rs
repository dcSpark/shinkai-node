use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::agent_serialization::SerializedAgent;
use crate::managers::IdentityManager;
use crate::schemas::identity::{
    DeviceIdentity, IdentityPermissions, IdentityType, StandardIdentity, StandardIdentityType,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rand::RngCore;
use rocksdb::{Error, Options};
use serde::{Serialize, Deserialize};
use serde_json::to_vec;
use shinkai_message_wasm::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_public_key_to_string_ref, string_to_encryption_public_key,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_public_key_to_string_ref, string_to_signature_public_key,
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(PartialEq, Debug)]
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

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub enum RegistrationCodeType {
    Device(String),
    Profile,
}

#[derive(PartialEq, Debug)]
pub struct RegistrationCodeInfo {
    pub status: RegistrationCodeStatus,
    pub permission: IdentityPermissions,
    pub code_type: RegistrationCodeType,
}

impl RegistrationCodeInfo {
    pub fn from_slice(slice: &[u8]) -> Self {
        let s = std::str::from_utf8(slice).unwrap();
        let parts: Vec<&str> = s.split(':').collect();
        let status = match parts.get(0) {
            Some(&"unused") => RegistrationCodeStatus::Unused,
            _ => RegistrationCodeStatus::Used,
        };
        let permission = match parts.get(1) {
            Some(&"admin") => IdentityPermissions::Admin,
            Some(&"standard") => IdentityPermissions::Standard,
            _ => IdentityPermissions::None,
        };
        let code_type = match parts.get(2) {
            Some(&"Device") => RegistrationCodeType::Device(parts.get(3).unwrap().to_string()),
            _ => RegistrationCodeType::Profile,
        };
        Self {
            status,
            permission,
            code_type,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match &self.code_type {
            RegistrationCodeType::Device(device_name) => format!(
                "{}:{}:{}:{}",
                match self.status {
                    RegistrationCodeStatus::Unused => "unused",
                    RegistrationCodeStatus::Used => "used",
                },
                match self.permission {
                    IdentityPermissions::Admin => "admin",
                    IdentityPermissions::Standard => "standard",
                    IdentityPermissions::None => "none",
                },
                "Device",
                device_name
            )
            .into_bytes(),
            RegistrationCodeType::Profile => format!(
                "{}:{}:{}",
                match self.status {
                    RegistrationCodeStatus::Unused => "unused",
                    RegistrationCodeStatus::Used => "used",
                },
                match self.permission {
                    IdentityPermissions::Admin => "admin",
                    IdentityPermissions::Standard => "standard",
                    IdentityPermissions::None => "none",
                },
                "Profile"
            )
            .into_bytes(),
        }
    }
}

impl ShinkaiDB {
    pub fn generate_registration_new_code(
        &self,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
    ) -> Result<String, Error> {
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = bs58::encode(random_bytes).into_string();

        let cf = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();

        let code_info = RegistrationCodeInfo {
            status: RegistrationCodeStatus::Unused,
            permission: permissions,
            code_type,
        };

        self.db.put_cf(cf, &new_code, &code_info.as_bytes())?;

        Ok(new_code)
    }

    pub fn use_registration_code(
        &self,
        registration_code: &str,
        node_name: &str,
        new_name: &str,
        identity_public_key: &str,
        encryption_public_key: &str,
    ) -> Result<(), ShinkaiDBError> {
        // Check if the code exists in Topic::OneTimeRegistrationCodes and its value is unused
        let cf_codes = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();
        let code_info: RegistrationCodeInfo = match self.db.get_cf(cf_codes, registration_code)? {
            Some(value) => RegistrationCodeInfo::from_slice(&value),
            None => return Err(ShinkaiDBError::CodeNonExistent),
        };

        if code_info.status != RegistrationCodeStatus::Unused {
            return Err(ShinkaiDBError::CodeAlreadyUsed);
        }

        if !new_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(ShinkaiDBError::InvalidProfileName(new_name.to_string()));
        }

        match code_info.code_type {
            RegistrationCodeType::Profile => {
                let current_identity_name =
                match ShinkaiName::from_node_and_profile(node_name.to_string(), new_name.to_lowercase()) {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(ShinkaiDBError::InvalidIdentityName(format!(
                            "{}/{}",
                            node_name, new_name
                        )))
                    }
                };

                match self.get_profile(current_identity_name.clone())? {
                    None => {
                        println!("current identity name: {}", current_identity_name);
                        let (node_encryption_public_key, node_signature_public_key) =
                            self.get_local_node_keys(current_identity_name)?;
                        let full_identity_name =
                            match ShinkaiName::from_node_and_profile(node_name.to_string(), new_name.to_string()) {
                                Ok(name) => name,
                                Err(_) => {
                                    return Err(ShinkaiDBError::InvalidIdentityName(format!(
                                        "{}/{}",
                                        node_name, new_name
                                    )))
                                }
                            };
                        let profile = StandardIdentity {
                            full_identity_name,
                            addr: None,
                            node_encryption_public_key,
                            node_signature_public_key,
                            profile_encryption_public_key: Some(string_to_encryption_public_key(
                                encryption_public_key,
                            )?),
                            profile_signature_public_key: Some(string_to_signature_public_key(identity_public_key)?),
                            identity_type: StandardIdentityType::Profile,
                            permission_type: code_info.permission.clone(),
                        };

                        println!("profile: {}", profile);
                        self.insert_profile(profile)?;
                    }
                    Some(_) => {
                        // Profile already exists, send an error
                        return Err(ShinkaiDBError::ProfileNameAlreadyExists);
                    }
                }
            }
            RegistrationCodeType::Device(profile_name) => {
                println!("profile name: {}", profile_name);
                let current_identity_name =
                match ShinkaiName::from_node_and_profile(node_name.to_string(), profile_name.to_lowercase()) {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(ShinkaiDBError::InvalidIdentityName(format!(
                            "{}/{}",
                            node_name, new_name
                        )))
                    }
                };

                println!("current identity name: {}", current_identity_name);

                match self.get_profile(current_identity_name.clone())? {
                    None => {
                        // send error. profile not found
                        return Err(ShinkaiDBError::ProfileNotFound(current_identity_name.to_string()));
                    }
                    Some(profile) => {
                        let full_identity_name =
                            match ShinkaiName::from_node_and_profile_and_type_and_name(node_name.to_string(), profile_name.to_string(), ShinkaiSubidentityType::Device, new_name.to_string()) {
                                Ok(name) => name,
                                Err(_) => {
                                    return Err(ShinkaiDBError::InvalidIdentityName(format!(
                                        "{}/{}",
                                        node_name, new_name
                                    )))
                                }
                            };

                        let device = DeviceIdentity {
                            full_identity_name,
                            node_encryption_public_key: profile.node_encryption_public_key,
                            node_signature_public_key: profile.node_signature_public_key,
                            profile_encryption_public_key: profile.profile_encryption_public_key,
                            profile_signature_public_key: profile.profile_signature_public_key,
                            device_signature_public_key: Some(string_to_signature_public_key(identity_public_key)?),
                            permission_type: code_info.permission,
                        };

                        println!("device: {:?}", device);
                        self.add_device_to_profile(device)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_code_info(
        &self,
        cf_codes: &rocksdb::ColumnFamily,
        registration_code: &str,
    ) -> Result<RegistrationCodeInfo, ShinkaiDBError> {
        match self.db.get_cf(cf_codes, registration_code)? {
            Some(value) => Ok(RegistrationCodeInfo::from_slice(&value)),
            None => return Err(ShinkaiDBError::CodeNonExistent),
        }
    }

    pub fn check_profile_existence(&self, profile_name: &str) -> Result<(), ShinkaiDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();

        if self.db.get_cf(cf_identity, profile_name)?.is_none() {
            return Err(ShinkaiDBError::ProfileNotFound(profile_name.to_string()));
        }

        Ok(())
    }

    pub fn update_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
        encryption_pk: EncryptionPublicKey,
        signature_pk: SignaturePublicKey,
    ) -> Result<(), ShinkaiDBError> {
        let node_name = my_node_identity_name.get_node_name().to_string();

        let cf_node_encryption = self.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
        let cf_node_identity = self.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();

        let mut batch = rocksdb::WriteBatch::default();

        // Convert public keys to bs58 encoded strings
        let encryption_pk_string = encryption_public_key_to_string(encryption_pk);
        let signature_pk_string = signature_public_key_to_string(signature_pk);

        batch.put_cf(
            cf_node_encryption,
            &node_name,
            encryption_pk_string.as_bytes(),
        );
        batch.put_cf(cf_node_identity, &node_name, signature_pk_string.as_bytes());

        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
    ) -> Result<(EncryptionPublicKey, SignaturePublicKey), ShinkaiDBError> {
        let node_name = my_node_identity_name.get_node_name().to_string();

        let cf_node_encryption = self.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
        let cf_node_identity = self.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();

        // Get the encryption key
        let encryption_pk_string = self
            .db
            .get_cf(cf_node_encryption, &node_name)?
            .ok_or(ShinkaiDBError::MissingValue(format!(
                "Missing encryption key for node {}",
                &my_node_identity_name
            )))?
            .to_vec();

        let encryption_pk = string_to_encryption_public_key(std::str::from_utf8(&encryption_pk_string)?)?;

        // Get the signature key
        let signature_pk_string = self
            .db
            .get_cf(cf_node_identity, &node_name)?
            .ok_or(ShinkaiDBError::MissingValue(format!(
                "Missing signature key for node {}",
                &my_node_identity_name
            )))?
            .to_vec();
        let signature_pk = string_to_signature_public_key(std::str::from_utf8(&signature_pk_string)?)?;

        Ok((encryption_pk, signature_pk))
    }
}
