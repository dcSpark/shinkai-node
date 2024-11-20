use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{identity::StandardIdentity, shinkai_name::ShinkaiName},
    shinkai_message::shinkai_message_schemas::IdentityPermissions,
    shinkai_utils::{
        encryption::{encryption_public_key_to_string_ref, string_to_encryption_public_key},
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
            .map_err(|e| SqliteManagerError::DatabaseError(e))?;
        Ok(count > 0)
    }

    pub fn get_all_profiles(
        &self,
        _my_node_identity: ShinkaiName,
    ) -> Result<Vec<StandardIdentity>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM standard_identities")?;
        let rows = stmt.query_map([], |row| {
            let full_identity_name: String = row.get(0)?;
            let addr: Option<Vec<u8>> = row.get(1)?;
            let node_encryption_public_key: Vec<u8> = row.get(2)?;
            let node_signature_public_key: Vec<u8> = row.get(3)?;
            let profile_encryption_public_key: Option<Vec<u8>> = row.get(4)?;
            let profile_signature_public_key: Option<Vec<u8>> = row.get(5)?;
            let identity_type: String = row.get(6)?;
            let permission_type: String = row.get(7)?;

            let node_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&node_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let node_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&node_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
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
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
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

    pub fn insert_profile(&self, identity: StandardIdentity) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO standard_identities (
                full_identity_name, addr,
                node_encryption_public_key,
                node_signature_public_key,
                profile_encryption_public_key,
                profile_signature_public_key,
                identity_type,
                permission_type
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        stmt.execute(params![
            identity.full_identity_name.full_name,
            identity.addr.map(|v| serde_json::to_vec(&v)).transpose()?,
            encryption_public_key_to_string_ref(&identity.node_encryption_public_key).as_bytes(),
            signature_public_key_to_string_ref(&identity.node_signature_public_key).as_bytes(),
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
        ])?;
        Ok(())
    }

    pub fn does_identity_exists(&self, profile: &ShinkaiName) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM standard_identities WHERE full_identity_name = ?")?;
        let count: i64 = stmt
            .query_row([profile.full_name.clone()], |row| row.get(0))
            .map_err(|e| SqliteManagerError::DatabaseError(e))?;
        Ok(count > 0)
    }

    pub fn get_profile_permission(&self, profile_name: ShinkaiName) -> Result<IdentityPermissions, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT permission_type FROM standard_identities WHERE full_identity_name = ?")?;
        let permission_type: String = stmt
            .query_row([profile_name.full_name], |row| row.get(0))
            .map_err(|e| SqliteManagerError::DatabaseError(e))?;
        Ok(serde_json::from_str(&permission_type)?)
    }

    pub fn get_device_permission(&self, device_name: ShinkaiName) -> Result<IdentityPermissions, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT permission_type FROM device_identities WHERE full_identity_name = ?")?;
        let permission_type: String = stmt
            .query_row([device_name.full_name], |row| row.get(0))
            .map_err(|e| SqliteManagerError::DatabaseError(e))?;
        Ok(serde_json::from_str(&permission_type)?)
    }

    pub fn remove_profile(&self, name: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM standard_identities WHERE full_identity_name = ?")?;
        stmt.execute([name])?;
        Ok(())
    }

    pub fn get_profile(&self, full_identity_name: ShinkaiName) -> Result<Option<StandardIdentity>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM standard_identities WHERE full_identity_name = ?")?;
        let profiles = stmt.query_map([full_identity_name.full_name], |row| {
            let full_identity_name: String = row.get(0)?;
            let addr: Option<Vec<u8>> = row.get(1)?;
            let node_encryption_public_key: Vec<u8> = row.get(2)?;
            let node_signature_public_key: Vec<u8> = row.get(3)?;
            let profile_encryption_public_key: Option<Vec<u8>> = row.get(4)?;
            let profile_signature_public_key: Option<Vec<u8>> = row.get(5)?;
            let identity_type: String = row.get(6)?;
            let permission_type: String = row.get(7)?;

            let node_encryption_public_key =
                string_to_encryption_public_key(std::str::from_utf8(&node_encryption_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let node_signature_public_key =
                string_to_signature_public_key(std::str::from_utf8(&node_signature_public_key)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
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
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
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
}
