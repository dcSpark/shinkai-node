use ed25519_dalek::VerifyingKey;
use rand::RngCore;
use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{
        identity::{DeviceIdentity, StandardIdentity, StandardIdentityType},
        identity_registration::{RegistrationCodeInfo, RegistrationCodeStatus},
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
    },
    shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType},
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, string_to_encryption_public_key},
        signatures::{signature_public_key_to_string, string_to_signature_public_key},
    },
};
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn generate_registration_new_code(
        &self,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
    ) -> Result<String, SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = hex::encode(random_bytes);

        let code_info = RegistrationCodeInfo {
            status: RegistrationCodeStatus::Unused,
            permission: permissions,
            code_type,
        };

        let mut stmt = conn.prepare("INSERT INTO registration_code (code, code_data) VALUES (?, ?)")?;
        stmt.execute(params![new_code, code_info.as_bytes()])?;

        Ok(new_code)
    }

    pub fn main_profile_exists(&self, node_name: &str) -> Result<bool, SqliteManagerError> {
        let profile_name = "main".to_string();
        let current_identity_name =
            match ShinkaiName::from_node_and_profile_names(node_name.to_string(), profile_name.to_lowercase()) {
                Ok(name) => name,
                Err(_) => {
                    return Err(SqliteManagerError::InvalidIdentityName(format!(
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
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("SELECT code_data FROM registration_code WHERE code = ?")?;
        let mut rows = stmt.query(params![registration_code])?;

        let code_info: RegistrationCodeInfo = match rows.next()? {
            Some(row) => {
                let code_data: Vec<u8> = row.get(0)?;
                RegistrationCodeInfo::from_slice(&code_data)
            }
            None => {
                return Err(SqliteManagerError::CodeNonExistent);
            }
        };

        if code_info.status != RegistrationCodeStatus::Unused {
            return Err(SqliteManagerError::CodeAlreadyUsed);
        }

        if !new_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(SqliteManagerError::InvalidProfileName(new_name.to_string()));
        }

        match code_info.code_type {
            RegistrationCodeType::Profile => {
                let current_identity_name =
                    match ShinkaiName::from_node_and_profile_names(node_name.to_string(), new_name.to_lowercase()) {
                        Ok(name) => name,
                        Err(_) => {
                            return Err(SqliteManagerError::InvalidIdentityName(format!(
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
                                    return Err(SqliteManagerError::InvalidIdentityName(format!(
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
                            profile_encryption_public_key: Some(
                                string_to_encryption_public_key(profile_encryption_public_key).map_err(|_| {
                                    SqliteManagerError::SomeError("Invalid profile encryption public key".to_string())
                                })?,
                            ),
                            profile_signature_public_key: Some(
                                string_to_signature_public_key(profile_identity_public_key).map_err(|_| {
                                    SqliteManagerError::SomeError("Invalid profile signature public key".to_string())
                                })?,
                            ),
                            identity_type: StandardIdentityType::Profile,
                            permission_type: code_info.permission.clone(),
                        };

                        self.insert_profile(profile)?;
                    }
                    Some(_) => {
                        // Profile already exists, send an error
                        return Err(SqliteManagerError::ProfileNameAlreadyExists);
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
                        return Err(SqliteManagerError::InvalidIdentityName(format!(
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
                                    return Err(SqliteManagerError::InvalidIdentityName(format!("{}/main", node_name)))
                                }
                            };

                        let main_profile = StandardIdentity {
                            full_identity_name,
                            addr: None,
                            node_encryption_public_key,
                            node_signature_public_key,
                            profile_encryption_public_key: Some(
                                string_to_encryption_public_key(profile_encryption_public_key).map_err(|_| {
                                    SqliteManagerError::SomeError("Invalid profile encryption public key".to_string())
                                })?,
                            ),
                            profile_signature_public_key: Some(
                                string_to_signature_public_key(profile_identity_public_key).map_err(|_| {
                                    SqliteManagerError::SomeError("Invalid profile signature public key".to_string())
                                })?,
                            ),
                            identity_type: StandardIdentityType::Profile,
                            permission_type: IdentityPermissions::Admin,
                        };

                        self.insert_profile(main_profile.clone())?;
                        main_profile
                    }
                    None => {
                        // send error. profile not found
                        return Err(SqliteManagerError::ProfileNotFound(current_identity_name.full_name));
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
                        return Err(SqliteManagerError::InvalidIdentityName(format!(
                            "{}/{}",
                            node_name, new_name
                        )))
                    }
                };

                let device_encryption_public_key = match device_encryption_public_key {
                    Some(key) => match string_to_encryption_public_key(key) {
                        Ok(parsed_key) => parsed_key,
                        Err(_) => {
                            return Err(SqliteManagerError::SomeError(
                                "Invalid device encryption public key".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(SqliteManagerError::SomeError(
                            "Device encryption public key is missing".to_string(),
                        ))
                    }
                };

                let device_signature_public_key = match device_identity_public_key {
                    Some(key) => match string_to_signature_public_key(key) {
                        Ok(parsed_key) => parsed_key,
                        Err(_) => {
                            return Err(SqliteManagerError::SomeError(
                                "Invalid device signature public key".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(SqliteManagerError::SomeError(
                            "Device signature public key is missing".to_string(),
                        ))
                    }
                };

                let profile_encryption_public_key = match profile.profile_encryption_public_key {
                    Some(key) => key,
                    None => {
                        return Err(SqliteManagerError::SomeError(
                            "Profile encryption public key is missing".to_string(),
                        ))
                    }
                };

                let profile_signature_public_key = match profile.profile_signature_public_key {
                    Some(key) => key,
                    None => {
                        return Err(SqliteManagerError::SomeError(
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

    pub fn get_registration_code_info(
        &self,
        registration_code: &str,
    ) -> Result<RegistrationCodeInfo, SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("SELECT code_data FROM registration_code WHERE code = ?")?;
        let mut rows = stmt.query(params![registration_code])?;

        match rows.next()? {
            Some(row) => {
                let code_data: Vec<u8> = row.get(0)?;
                Ok(RegistrationCodeInfo::from_slice(&code_data))
            }
            None => Err(SqliteManagerError::CodeNonExistent),
        }
    }

    pub fn update_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
        encryption_pk: EncryptionPublicKey,
        signature_pk: VerifyingKey,
    ) -> Result<(), SqliteManagerError> {
        let node_name = my_node_identity_name.get_node_name_string().to_string();

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO local_node_keys (node_name, node_encryption_public_key, node_signature_public_key) VALUES (?, ?, ?)",
        )?;
        stmt.execute(params![
            node_name,
            encryption_public_key_to_string(encryption_pk),
            signature_public_key_to_string(signature_pk)
        ])?;

        Ok(())
    }

    pub fn get_local_node_keys(
        &self,
        my_node_identity_name: ShinkaiName,
    ) -> Result<(EncryptionPublicKey, VerifyingKey), SqliteManagerError> {
        let node_name = my_node_identity_name.get_node_name_string().to_string();

        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT node_encryption_public_key, node_signature_public_key FROM local_node_keys WHERE node_name = ?",
        )?;
        let mut rows = stmt.query(params![node_name])?;

        match rows.next()? {
            Some(row) => {
                let node_encryption_public_key: Vec<u8> = row.get(0)?;
                let node_signature_public_key: Vec<u8> = row.get(1)?;

                Ok((
                    string_to_encryption_public_key(std::str::from_utf8(&node_encryption_public_key).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?)
                    .map_err(|_| SqliteManagerError::SomeError("Invalid node encryption public key".to_string()))?,
                    string_to_signature_public_key(std::str::from_utf8(&node_signature_public_key).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?)
                    .map_err(|_| SqliteManagerError::SomeError("Invalid node signature public key".to_string()))?,
                ))
            }
            None => Err(SqliteManagerError::MissingValue(format!(
                "Missing encryption and signature keys for node {}",
                &node_name
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_generate_and_get_registration_code() {
        let manager = setup_test_db();

        let permissions = IdentityPermissions::Admin;
        let code_type = RegistrationCodeType::Profile;

        let code = manager
            .generate_registration_new_code(permissions.clone(), code_type.clone())
            .unwrap();
        let code_info = manager.get_registration_code_info(&code).unwrap();

        assert_eq!(code_info.permission, permissions);
        assert_eq!(code_info.code_type, code_type);
    }
}
