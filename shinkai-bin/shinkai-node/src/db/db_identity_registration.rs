use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::schemas::identity::{DeviceIdentity, StandardIdentity, StandardIdentityType};
use ed25519_dalek::VerifyingKey;
use rand::RngCore;
use shinkai_message_primitives::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, string_to_encryption_public_key,
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    signature_public_key_to_string, string_to_signature_public_key,
};
use x25519_dalek::{PublicKey as EncryptionPublicKey};

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
        let status = match parts.first() {
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
    ) -> Result<String, ShinkaiDBError> {
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = hex::encode(random_bytes);

        let cf = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;

        let code_info = RegistrationCodeInfo {
            status: RegistrationCodeStatus::Unused,
            permission: permissions,
            code_type,
        };

        let prefixed_new_code = format!("registration_code_{}", new_code);

        self.db
            .put_cf(cf, prefixed_new_code.as_bytes(), code_info.as_bytes())?;

        Ok(new_code)
    }

    pub fn main_profile_exists(&self, node_name: &str) -> Result<bool, ShinkaiDBError> {
        let profile_name = "main".to_string();
        let current_identity_name =
            match ShinkaiName::from_node_and_profile_names(node_name.to_string(), profile_name.to_lowercase()) {
                Ok(name) => name,
                Err(_) => {
                    return Err(ShinkaiDBError::InvalidIdentityName(format!(
                        "{}/{}",
                        node_name, profile_name
                    )))
                }
            };

        match self.get_profile(current_identity_name.clone())? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub fn use_registration_code(
        &self,
        registration_code: &str,
        node_name: &str,
        new_name: &str,
        profile_identity_public_key: &str,
        profile_encryption_public_key: &str,
        device_identity_public_key: Option<&str>,
        device_encryption_public_key: Option<&str>,
    ) -> Result<(), ShinkaiDBError> {
        let cf_codes = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;
        let prefixed_registration_code = format!("registration_code_{}", registration_code);
        let code_info: RegistrationCodeInfo = match self.db.get_cf(cf_codes, prefixed_registration_code.as_bytes())? {
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
                    match ShinkaiName::from_node_and_profile_names(node_name.to_string(), new_name.to_lowercase()) {
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
                        let (node_encryption_public_key, node_signature_public_key) =
                            self.get_local_node_keys(current_identity_name)?;
                        let full_identity_name =
                            match ShinkaiName::from_node_and_profile_names(node_name.to_string(), new_name.to_string())
                            {
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
                                profile_encryption_public_key,
                            )?),
                            profile_signature_public_key: Some(string_to_signature_public_key(
                                profile_identity_public_key,
                            )?),
                            identity_type: StandardIdentityType::Profile,
                            permission_type: code_info.permission.clone(),
                        };

                        self.insert_profile(profile)?;
                    }
                    Some(_) => {
                        // Profile already exists, send an error
                        return Err(ShinkaiDBError::ProfileNameAlreadyExists);
                    }
                }
            }
            RegistrationCodeType::Device(profile_name) => {
                let current_identity_name = match ShinkaiName::from_node_and_profile_names(
                    node_name.to_string(),
                    profile_name.to_lowercase(),
                ) {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(ShinkaiDBError::InvalidIdentityName(format!(
                            "{}/{}",
                            node_name, new_name
                        )))
                    }
                };

                let profile = match self.get_profile(current_identity_name.clone())? {
                    None if profile_name == "main" => {
                        // Create main profile
                        let (node_encryption_public_key, node_signature_public_key) =
                            self.get_local_node_keys(current_identity_name)?;

                        let full_identity_name =
                            match ShinkaiName::from_node_and_profile_names(node_name.to_string(), "main".to_string()) {
                                Ok(name) => name,
                                Err(_) => {
                                    return Err(ShinkaiDBError::InvalidIdentityName(format!("{}/main", node_name)))
                                }
                            };

                        let main_profile = StandardIdentity {
                            full_identity_name,
                            addr: None,
                            node_encryption_public_key,
                            node_signature_public_key,
                            profile_encryption_public_key: Some(string_to_encryption_public_key(
                                profile_encryption_public_key,
                            )?),
                            profile_signature_public_key: Some(string_to_signature_public_key(
                                profile_identity_public_key,
                            )?),
                            identity_type: StandardIdentityType::Profile,
                            permission_type: IdentityPermissions::Admin,
                        };

                        self.insert_profile(main_profile.clone())?;
                        main_profile
                    }
                    None => {
                        // send error. profile not found
                        return Err(ShinkaiDBError::ProfileNotFound(current_identity_name.to_string()));
                    }
                    Some(existing_profile) => existing_profile,
                };

                let full_identity_name = match ShinkaiName::from_node_and_profile_names_and_type_and_name(
                    node_name.to_string(),
                    profile_name.to_string(),
                    ShinkaiSubidentityType::Device,
                    new_name.to_string(),
                ) {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(ShinkaiDBError::InvalidIdentityName(format!(
                            "{}/{}",
                            node_name, new_name
                        )))
                    }
                };

                let device_encryption_public_key = match device_encryption_public_key {
                    Some(key) => match string_to_encryption_public_key(key) {
                        Ok(parsed_key) => parsed_key,
                        Err(_) => {
                            return Err(ShinkaiDBError::SomeError(
                                "Invalid device encryption public key".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(ShinkaiDBError::SomeError(
                            "Device encryption public key is missing".to_string(),
                        ))
                    }
                };

                let device_signature_public_key = match device_identity_public_key {
                    Some(key) => match string_to_signature_public_key(key) {
                        Ok(parsed_key) => parsed_key,
                        Err(_) => {
                            return Err(ShinkaiDBError::SomeError(
                                "Invalid device signature public key".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(ShinkaiDBError::SomeError(
                            "Device signature public key is missing".to_string(),
                        ))
                    }
                };

                let profile_encryption_public_key = match profile.profile_encryption_public_key {
                    Some(key) => key,
                    None => {
                        return Err(ShinkaiDBError::SomeError(
                            "Profile encryption public key is missing".to_string(),
                        ))
                    }
                };

                let profile_signature_public_key = match profile.profile_signature_public_key {
                    Some(key) => key,
                    None => {
                        return Err(ShinkaiDBError::SomeError(
                            "Profile signature public key is missing".to_string(),
                        ))
                    }
                };

                let device = DeviceIdentity {
                    full_identity_name: full_identity_name.clone(),
                    node_encryption_public_key: profile.node_encryption_public_key,
                    node_signature_public_key: profile.node_signature_public_key,
                    profile_encryption_public_key,
                    profile_signature_public_key,
                    device_encryption_public_key,
                    device_signature_public_key,
                    permission_type: code_info.permission,
                };

                self.add_device_to_profile(device)?;
            }
        }

        Ok(())
    }

    pub fn get_registration_code_info(&self, registration_code: &str) -> Result<RegistrationCodeInfo, ShinkaiDBError> {
        let cf_codes = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;

        let prefixed_registration_code = format!("registration_code_{}", registration_code);
        match self.db.get_cf(cf_codes, prefixed_registration_code.as_bytes())? {
            Some(value) => Ok(RegistrationCodeInfo::from_slice(&value)),
            None => Err(ShinkaiDBError::CodeNonExistent),
        }
    }

    pub fn check_profile_existence(&self, profile_name: &str) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;

        // Check if the profile exists by looking for its associated encryption public key
        let encryption_key_prefix = format!("encryption_key_of_{}", profile_name);
        match self.db.get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())? {
            Some(_) => Ok(()),
            None => Err(ShinkaiDBError::ProfileNotFound(profile_name.to_string())),
        }
    }

    pub fn update_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
        encryption_pk: EncryptionPublicKey,
        signature_pk: VerifyingKey,
    ) -> Result<(), ShinkaiDBError> {
        let node_name = my_node_identity_name.get_node_name_string().to_string();

        // Use Topic::NodeAndUsers with appropriate prefixes for node keys
        let cf_node_and_users = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;

        let mut batch = rocksdb::WriteBatch::default();

        // Convert public keys to hex encoded strings
        let encryption_pk_string = encryption_public_key_to_string(encryption_pk);
        let signature_pk_string = signature_public_key_to_string(signature_pk);

        // Use specific prefixes for encryption and signature public keys
        let encryption_key_prefix = format!("node_encryption_key_{}", node_name);
        let signature_key_prefix = format!("node_signature_key_{}", node_name);

        batch.put_cf(
            cf_node_and_users,
            encryption_key_prefix.as_bytes(),
            encryption_pk_string.as_bytes(),
        );
        batch.put_cf(
            cf_node_and_users,
            signature_key_prefix.as_bytes(),
            signature_pk_string.as_bytes(),
        );

        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
    ) -> Result<(EncryptionPublicKey, VerifyingKey), ShinkaiDBError> {
        let node_name = my_node_identity_name.get_node_name_string().to_string();

        // Use Topic::NodeAndUsers for both encryption and signature keys with specific prefixes
        let cf_node_and_users = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::SomeError(
                "Column family NodeAndUsers not found".to_string(),
            ))?;

        // Prefixes for node encryption and signature keys
        let encryption_key_prefix = format!("node_encryption_key_{}", node_name);
        let signature_key_prefix = format!("node_signature_key_{}", node_name);

        // Get the encryption key
        let encryption_pk_string = self
            .db
            .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())?
            .ok_or(ShinkaiDBError::MissingValue(format!(
                "Missing encryption key for node {}",
                &node_name
            )))?
            .to_vec();

        let encryption_pk = string_to_encryption_public_key(std::str::from_utf8(&encryption_pk_string)?)?;

        // Get the signature key
        let signature_pk_string = self
            .db
            .get_cf(cf_node_and_users, signature_key_prefix.as_bytes())?
            .ok_or(ShinkaiDBError::MissingValue(format!(
                "Missing signature key for node {}",
                &my_node_identity_name
            )))?
            .to_vec();

        let signature_pk = string_to_signature_public_key(std::str::from_utf8(&signature_pk_string)?)?;

        Ok((encryption_pk, signature_pk))
    }
}
