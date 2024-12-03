use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{
        identity::{DeviceIdentity, Identity, StandardIdentity},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::IdentityPermissions,
    shinkai_utils::{
        encryption::{encryption_public_key_to_string_ref, string_to_encryption_public_key},
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::{signature_public_key_to_string_ref, string_to_signature_public_key},
    },
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn has_any_profile(&self) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM standard_identities")?;
        let count: i64 = stmt
            .query_row([], |row| row.get(0))
            .map_err(SqliteManagerError::DatabaseError)?;
        Ok(count > 0)
    }

    pub fn get_all_profiles_and_devices(
        &self,
        my_node_identity: ShinkaiName,
    ) -> Result<Vec<Identity>, SqliteManagerError> {
        let standard_identities = self.get_all_profiles(my_node_identity.clone())?;
        let devices = self.get_all_devices(my_node_identity)?;

        let identities: Vec<Identity> = standard_identities
            .into_iter()
            .map(Identity::Standard)
            .chain(devices.into_iter().map(Identity::Device))
            .collect();

        Ok(identities)
    }

    pub fn get_all_profiles(&self, my_node_identity: ShinkaiName) -> Result<Vec<StandardIdentity>, SqliteManagerError> {
        let my_node_identity_name = my_node_identity.get_node_name_string();

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM standard_identities")?;
        let rows = stmt.query_map([], |row| {
            let profile_name: String = row.get(0)?;
            let addr: Option<Vec<u8>> = row.get(1)?;
            let profile_encryption_public_key: Option<Vec<u8>> = row.get(2)?;
            let profile_signature_public_key: Option<Vec<u8>> = row.get(3)?;
            let identity_type: String = row.get(4)?;
            let permission_type: String = row.get(5)?;

            let full_identity_name =
                ShinkaiName::from_node_and_profile_names(my_node_identity_name.clone(), profile_name.to_string())
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?;

            let (node_encryption_public_key, node_signature_public_key) =
                self.get_local_node_keys(full_identity_name.clone()).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
                        e.to_string(),
                    ))
                })?;

            let profile_encryption_public_key = profile_encryption_public_key
                .map(|v| {
                    string_to_encryption_public_key(std::str::from_utf8(&v)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })
                })
                .transpose()?;
            let profile_signature_public_key = profile_signature_public_key
                .map(|v| {
                    string_to_signature_public_key(std::str::from_utf8(&v)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })
                })
                .transpose()?;

            Ok(StandardIdentity {
                full_identity_name,
                addr: addr
                    .map(|v| {
                        serde_json::from_slice(&v).map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                                e.to_string(),
                            )))
                        })
                    })
                    .transpose()?,
                node_encryption_public_key,
                node_signature_public_key,
                profile_encryption_public_key,
                profile_signature_public_key,
                identity_type: serde_json::from_str(&identity_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                permission_type: serde_json::from_str(&permission_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;
        let mut identities = Vec::new();
        for identity in rows {
            identities.push(identity?);
        }
        Ok(identities)
    }

    pub fn get_profile(&self, full_identity_name: ShinkaiName) -> Result<Option<StandardIdentity>, SqliteManagerError> {
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(full_identity_name.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM standard_identities WHERE profile_name = ?")?;
        let profiles = stmt.query_map([profile_name], |row| {
            let addr: Option<Vec<u8>> = row.get(1)?;
            let profile_encryption_public_key: Option<Vec<u8>> = row.get(2)?;
            let profile_signature_public_key: Option<Vec<u8>> = row.get(3)?;
            let identity_type: String = row.get(4)?;
            let permission_type: String = row.get(5)?;

            let (node_encryption_public_key, node_signature_public_key) =
                self.get_local_node_keys(full_identity_name.clone()).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
                        e.to_string(),
                    ))
                })?;

            let profile_encryption_public_key = profile_encryption_public_key
                .map(|v| {
                    string_to_encryption_public_key(std::str::from_utf8(&v)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })
                })
                .transpose()?;
            let profile_signature_public_key = profile_signature_public_key
                .map(|v| {
                    string_to_signature_public_key(std::str::from_utf8(&v)?).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })
                })
                .transpose()?;

            Ok(StandardIdentity {
                full_identity_name: full_identity_name.clone(),
                addr: addr
                    .map(|v| {
                        serde_json::from_slice(&v).map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                                e.to_string(),
                            )))
                        })
                    })
                    .transpose()?,
                node_encryption_public_key,
                node_signature_public_key,
                profile_encryption_public_key,
                profile_signature_public_key,
                identity_type: serde_json::from_str(&identity_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                permission_type: serde_json::from_str(&permission_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        let mut results = Vec::new();
        for profile in profiles {
            results.push(profile?);
        }

        Ok(results.pop())
    }

    pub fn insert_profile(&self, identity: StandardIdentity) -> Result<(), SqliteManagerError> {
        let profile_name =
            identity
                .full_identity_name
                .get_profile_name_string()
                .ok_or(SqliteManagerError::InvalidIdentityName(
                    identity.full_identity_name.to_string(),
                ))?;

        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO standard_identities (
                profile_name,
                addr,
                profile_encryption_public_key,
                profile_signature_public_key,
                identity_type,
                permission_type
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                profile_name,
                identity.addr.map(|v| serde_json::to_vec(&v)).transpose()?,
                identity
                    .profile_encryption_public_key
                    .map(|v| encryption_public_key_to_string_ref(&v))
                    .unwrap_or_default()
                    .as_bytes(),
                identity
                    .profile_signature_public_key
                    .map(|v| signature_public_key_to_string_ref(&v))
                    .unwrap_or_default()
                    .as_bytes(),
                serde_json::to_string(&identity.identity_type)?,
                serde_json::to_string(&identity.permission_type)?,
            ],
        )?;

        let node_name = identity.full_identity_name.get_node_name_string().to_string();

        tx.execute(
            "INSERT OR REPLACE INTO local_node_keys (node_name, node_encryption_public_key, node_signature_public_key) VALUES (?, ?, ?)",
        params![
            node_name,
            encryption_public_key_to_string_ref(&identity.node_encryption_public_key).as_bytes(),
            signature_public_key_to_string_ref(&identity.node_signature_public_key).as_bytes(),
        ])?;

        tx.commit()?;
        Ok(())
    }

    pub fn does_identity_exist(&self, profile: &ShinkaiName) -> Result<bool, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM standard_identities WHERE profile_name = ?")?;
        let count: i64 = stmt
            .query_row([profile_name], |row| row.get(0))
            .map_err(SqliteManagerError::DatabaseError)?;
        Ok(count > 0)
    }

    pub fn get_profile_permission(&self, profile: ShinkaiName) -> Result<IdentityPermissions, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT permission_type FROM standard_identities WHERE profile_name = ?")?;
        let permission_type: String = stmt
            .query_row([profile_name], |row| row.get(0))
            .map_err(SqliteManagerError::DatabaseError)?;
        Ok(serde_json::from_str(&permission_type)?)
    }

    pub fn get_device_permission(&self, device: ShinkaiName) -> Result<IdentityPermissions, SqliteManagerError> {
        let device_name = device
            .get_fullname_string_without_node_name()
            .ok_or(SqliteManagerError::InvalidIdentityName(device.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT permission_type FROM device_identities WHERE device_name = ?")?;
        let permission_type: String = stmt
            .query_row([device_name], |row| row.get(0))
            .map_err(SqliteManagerError::DatabaseError)?;
        Ok(serde_json::from_str(&permission_type)?)
    }

    pub fn remove_profile(&self, name: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM standard_identities WHERE profile_name = ?")?;
        stmt.execute([name])?;
        Ok(())
    }

    pub fn get_device(&self, full_identity_name: ShinkaiName) -> Result<DeviceIdentity, SqliteManagerError> {
        let device_name = full_identity_name
            .get_fullname_string_without_node_name()
            .ok_or(SqliteManagerError::InvalidIdentityName(full_identity_name.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM device_identities WHERE device_name = ?")?;
        let device = stmt.query_row([device_name], |row| {
            let profile_encryption_public_key: Vec<u8> = row.get(1)?;
            let profile_signature_public_key: Vec<u8> = row.get(2)?;
            let device_encryption_public_key: Vec<u8> = row.get(3)?;
            let device_signature_public_key: Vec<u8> = row.get(4)?;
            let permission_type: String = row.get(5)?;

            let (node_encryption_public_key, node_signature_public_key) =
                self.get_local_node_keys(full_identity_name.clone()).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
                        e.to_string(),
                    ))
                })?;
            let profile_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&profile_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let profile_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&profile_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let device_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&device_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let device_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&device_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

            Ok(DeviceIdentity {
                full_identity_name: full_identity_name.clone(),
                node_encryption_public_key,
                node_signature_public_key,
                profile_encryption_public_key,
                profile_signature_public_key,
                device_encryption_public_key,
                device_signature_public_key,
                permission_type: serde_json::from_str(&permission_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        Ok(device)
    }

    pub fn get_all_devices(&self, my_node_identity: ShinkaiName) -> Result<Vec<DeviceIdentity>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM device_identities")?;
        let rows = stmt.query_map([], |row| {
            let profile_encryption_public_key: Vec<u8> = row.get(1)?;
            let profile_signature_public_key: Vec<u8> = row.get(2)?;
            let device_encryption_public_key: Vec<u8> = row.get(3)?;
            let device_signature_public_key: Vec<u8> = row.get(4)?;
            let permission_type: String = row.get(5)?;

            let full_identity_name = my_node_identity.clone();

            let (node_encryption_public_key, node_signature_public_key) =
                self.get_local_node_keys(full_identity_name.clone()).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
                        e.to_string(),
                    ))
                })?;
            let profile_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&profile_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let profile_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&profile_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let device_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&device_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let device_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&device_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

            Ok(DeviceIdentity {
                full_identity_name,
                node_encryption_public_key,
                node_signature_public_key,
                profile_encryption_public_key,
                profile_signature_public_key,
                device_encryption_public_key,
                device_signature_public_key,
                permission_type: serde_json::from_str(&permission_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        let mut devices = Vec::new();
        for device in rows {
            devices.push(device?);
        }

        Ok(devices)
    }

    pub fn add_device_to_profile(&self, device: DeviceIdentity) -> Result<(), SqliteManagerError> {
        {
            let conn = self.get_connection()?;
            let mut stmt = conn.prepare("SELECT profile_name FROM standard_identities")?;

            let profile_name = match device.full_identity_name.get_profile_name_string() {
                Some(name) => name,
                None => {
                    return Err(SqliteManagerError::InvalidIdentityName(
                        device.full_identity_name.to_string(),
                    ))
                }
            };

            let profile_names = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, rusqlite::Error>>()?;

            // Check if the profile exists
            if !profile_names.iter().any(|name| *name == profile_name) {
                return Err(SqliteManagerError::ProfileNotFound(profile_name));
            }
        }

        let device_name = device
            .full_identity_name
            .get_fullname_string_without_node_name()
            .ok_or(SqliteManagerError::InvalidIdentityName(
                device.full_identity_name.to_string(),
            ))?;

        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO device_identities (
                device_name,
                profile_encryption_public_key,
                profile_signature_public_key,
                device_encryption_public_key,
                device_signature_public_key,
                permission_type
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                device_name,
                encryption_public_key_to_string_ref(&device.profile_encryption_public_key).as_bytes(),
                signature_public_key_to_string_ref(&device.profile_signature_public_key).as_bytes(),
                encryption_public_key_to_string_ref(&device.device_encryption_public_key).as_bytes(),
                signature_public_key_to_string_ref(&device.device_signature_public_key).as_bytes(),
                serde_json::to_string(&device.permission_type)?,
            ],
        )?;

        let node_name = device.full_identity_name.get_node_name_string().to_string();

        tx.execute(
            "INSERT OR REPLACE INTO local_node_keys (node_name, node_encryption_public_key, node_signature_public_key) VALUES (?, ?, ?)",
        params![
            node_name,
            encryption_public_key_to_string_ref(&device.node_encryption_public_key).as_bytes(),
            signature_public_key_to_string_ref(&device.node_signature_public_key).as_bytes(),
        ])?;

        tx.commit()?;

        Ok(())
    }

    pub fn debug_print_all_keys_for_profiles_identity_key(&self) {
        let conn = match self.get_connection() {
            Ok(conn) => conn,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("Error reading from database: {}", e).as_str(),
                );
                return;
            }
        };

        // Print all standard identities
        let mut stmt = match conn.prepare("SELECT profile_name FROM standard_identities") {
            Ok(stmt) => stmt,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("Error reading from database: {}", e).as_str(),
                );
                return;
            }
        };

        let identity_names = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(identity_names) => identity_names,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("Error reading from database: {}", e).as_str(),
                );
                return;
            }
        };

        let identity_names = identity_names.filter_map(|id| id.ok()).collect::<Vec<_>>();

        for identity_name in identity_names {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Debug,
                format!("print_all_keys_for_profiles_identity_key {}", identity_name).as_str(),
            );
        }

        // Print all device identities
        let mut stmt = match conn.prepare("SELECT device_name FROM device_identities") {
            Ok(stmt) => stmt,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("Error reading from database: {}", e).as_str(),
                );
                return;
            }
        };

        let identity_names = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(identity_names) => identity_names,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("Error reading from database: {}", e).as_str(),
                );
                return;
            }
        };

        let identity_names = identity_names.filter_map(|id| id.ok()).collect::<Vec<_>>();

        for identity_name in identity_names {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Debug,
                format!("print_all_devices_for_profile {}", identity_name).as_str(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::{
        schemas::{identity::StandardIdentityType, shinkai_name::ShinkaiName},
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, signatures::unsafe_deterministic_signature_keypair,
        },
    };
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
    fn test_insert_and_get_profile() {
        let manager = setup_test_db();

        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let profile = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        manager.insert_profile(profile.clone()).unwrap();

        let retrieved_profile = manager
            .get_profile(profile.full_identity_name.clone())
            .unwrap()
            .unwrap();

        assert_eq!(profile, retrieved_profile);
    }

    #[test]
    fn test_get_all_profiles() {
        let manager = setup_test_db();

        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let profile1 = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        let (_, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let profile2 = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node2".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node2_encryption_pk),
            profile_signature_public_key: Some(node2_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        manager.insert_profile(profile1.clone()).unwrap();
        manager.insert_profile(profile2.clone()).unwrap();

        let profiles = manager
            .get_all_profiles(ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap())
            .unwrap();

        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains(&profile1));
        assert!(profiles.contains(&profile2));
    }

    #[test]
    fn test_remove_profile() {
        let manager = setup_test_db();

        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let full_identity_name = ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap();
        let profile_name = full_identity_name.get_profile_name_string().unwrap_or_default();

        let profile = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        manager.insert_profile(profile.clone()).unwrap();

        let retrieved_profile = manager
            .get_profile(profile.full_identity_name.clone())
            .unwrap()
            .unwrap();

        assert_eq!(profile, retrieved_profile);

        manager.remove_profile(&profile_name).unwrap();

        let retrieved_profile = manager.get_profile(profile.full_identity_name.clone()).unwrap();

        assert!(retrieved_profile.is_none());

        let has_any_profile = manager.has_any_profile().unwrap();
        assert!(!has_any_profile);
    }

    #[test]
    fn test_add_device_to_profile() {
        let manager = setup_test_db();

        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let profile = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        manager.insert_profile(profile.clone()).unwrap();

        let (_, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let device = DeviceIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            node_encryption_public_key: node2_encryption_pk,
            node_signature_public_key: node2_identity_pk,
            profile_encryption_public_key: node1_encryption_pk,
            profile_signature_public_key: node1_identity_pk,
            device_encryption_public_key: node2_encryption_pk,
            device_signature_public_key: node2_identity_pk,
            permission_type: IdentityPermissions::Admin,
        };

        manager.add_device_to_profile(device.clone()).unwrap();

        let retrieved_device = manager.get_device(device.full_identity_name.clone()).unwrap();

        assert_eq!(device, retrieved_device);
    }

    #[test]
    fn test_get_all_profiles_and_devices() {
        let manager = setup_test_db();

        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let profile = StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        };

        manager.insert_profile(profile.clone()).unwrap();

        let (_, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let device = DeviceIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: node1_encryption_pk,
            profile_signature_public_key: node1_identity_pk,
            device_encryption_public_key: node2_encryption_pk,
            device_signature_public_key: node2_identity_pk,
            permission_type: IdentityPermissions::Admin,
        };

        manager.add_device_to_profile(device.clone()).unwrap();

        let identities = manager
            .get_all_profiles_and_devices(ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap())
            .unwrap();

        assert_eq!(identities.len(), 2);
        assert!(identities.contains(&Identity::Standard(profile.clone())));
        assert!(identities.contains(&Identity::Device(device.clone())));
    }
}
