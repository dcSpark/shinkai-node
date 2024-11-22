use std::str::FromStr;

use rusqlite::params;
use shinkai_message_primitives::schemas::{
    identity::StandardIdentity, inbox_permission::InboxPermission, shinkai_name::ShinkaiName,
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn create_empty_inbox(&self, inbox_name: String) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inboxes (inbox_name, smart_inbox_name) VALUES (?1, ?1)",
            params![inbox_name],
        )?;
        Ok(())
    }

    pub fn does_inbox_exist(&self, inbox_name: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM inboxes WHERE inbox_name = ?1")?;
        let mut rows = stmt.query(params![inbox_name])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        Ok(count > 0)
    }

    pub fn mark_as_read_up_to(
        &self,
        inbox_name: String,
        up_to_message_hash_offset: String,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE inboxes SET read_up_to_message_hash = ?1 WHERE inbox_name = ?2",
            params![up_to_message_hash_offset, inbox_name],
        )?;
        Ok(())
    }

    pub fn add_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<(), SqliteManagerError> {
        let shinkai_profile = identity
            .full_identity_name
            .extract_profile()
            .map_err(|_| SqliteManagerError::InvalidProfileName(identity.full_identity_name.to_string()))?;
        self.add_permission_with_profile(inbox_name, shinkai_profile, perm)
    }

    pub fn add_permission_with_profile(
        &self,
        inbox_name: &str,
        profile: ShinkaiName,
        perm: InboxPermission,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inbox_profile_permissions (inbox_name, profile_name, permission) VALUES (?1, ?2, ?3)",
            params![inbox_name, profile.to_string(), perm.to_string()],
        )?;
        Ok(())
    }

    pub fn remove_permission(&self, inbox_name: &str, identity: &StandardIdentity) -> Result<(), SqliteManagerError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            SqliteManagerError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        let profile_exists = self.does_identity_exist(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(SqliteManagerError::ProfileNotFound(profile_name));
        }

        if !self.does_inbox_exist(inbox_name)? {
            return Err(SqliteManagerError::InboxNotFound(inbox_name.to_string()));
        }

        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM inbox_profile_permissions WHERE inbox_name = ?1 AND profile_name = ?2",
            params![inbox_name, profile_name],
        )?;

        Ok(())
    }

    pub fn has_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<bool, SqliteManagerError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            SqliteManagerError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        let profile_exists = self.does_identity_exist(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(SqliteManagerError::ProfileNotFound(profile_name));
        }

        if !self.does_inbox_exist(inbox_name)? {
            return Err(SqliteManagerError::InboxNotFound(inbox_name.to_string()));
        }

        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT permission FROM inbox_profile_permissions WHERE inbox_name = ?1 AND profile_name = ?2")?;
        let stored_permission = stmt.query_row(params![inbox_name, profile_name], |row| {
            let perm_str: String = row.get(0)?;
            let permission = InboxPermission::from_str(&perm_str).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            Ok(permission)
        })?;

        Ok(stored_permission >= perm)
    }

    pub fn update_smart_inbox_name(&self, inbox_id: &str, new_name: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE inboxes SET smart_inbox_name = ?1 WHERE inbox_name = ?2",
            params![new_name, inbox_id],
        )?;
        Ok(())
    }
}
