use super::{db_errors::ShinkaiDBError, db_main::Topic, ShinkaiDB};
use crate::schemas::identity::{DeviceIdentity, Identity, StandardIdentity, StandardIdentityType};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string_ref, string_to_encryption_public_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::{
    signature_public_key_to_string_ref, string_to_signature_public_key,
};
use x25519_dalek::PublicKey as EncryptionPublicKey;

impl ShinkaiDB {
    pub fn get_encryption_public_key(&self, identity_public_key: &str) -> Result<String, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Use a prefix based on the current Topic for the identity public key
        let profile_key_prefix = format!("profile_from_identity_key_{}", identity_public_key);
        // Get the associated profile name for the identity public key
        let profile_name = match self.db.get_cf(cf_node_and_users, profile_key_prefix.as_bytes())? {
            Some(name_bytes) => Ok(String::from_utf8_lossy(&name_bytes).to_string()),
            None => Err(ShinkaiDBError::ProfileNameNonExistent(identity_public_key.to_string())),
        }?;

        // Use "encryption_key_of_{}" with the profile name to get the encryption key
        let encryption_key_prefix = format!("encryption_key_of_{}", profile_name);
        // Get the associated encryption public key for the profile name
        match self.db.get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())? {
            Some(encryption_key_bytes) => Ok(String::from_utf8_lossy(&encryption_key_bytes).to_string()),
            None => Err(ShinkaiDBError::EncryptionKeyNonExistent),
        }
    }

    pub fn has_any_profile(&self) -> Result<bool, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        // Use Topic::NodeAndUsers for profiles related information with specific prefixes
        let iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    // Check if the key starts with the specific prefix for profiles
                    if key_str.starts_with("identity_key_of_") {
                        return Ok(true); // Return true upon finding the first profile
                    }
                }
                Err(_) => continue, // Optionally handle the error, for example, by continuing to the next item
            }
        }

        Ok(false) // Return false if no profiles are found
    }

    pub fn get_all_profiles(&self, my_node_identity: ShinkaiName) -> Result<Vec<StandardIdentity>, ShinkaiDBError> {
        let my_node_identity_name = my_node_identity.get_node_name_string();

        // Use Topic::NodeAndUsers for profiles related information with specific prefixes
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let (node_encryption_public_key, node_signature_public_key) =
            self.get_local_node_keys(my_node_identity.clone())?;

        let mut result = Vec::new();
        let iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    // Filter out profiles based on prefix
                    if key_str.starts_with("identity_key_of_") {
                        let profile_name = key_str.trim_start_matches("identity_key_of_");
                        let full_identity_name = ShinkaiName::from_node_and_profile_names(
                            my_node_identity_name.clone(),
                            profile_name.to_string(),
                        )?;
                        let subidentity_signature_public_key =
                            string_to_signature_public_key(&String::from_utf8(value.to_vec()).unwrap())
                                .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

                        // Retrieve subidentity encryption public key, identity type, and permissions using specific prefixes
                        let subidentity_encryption_public_key =
                            self.get_subidentity_encryption_public_key(full_identity_name.clone())?;
                        let identity_type = self.get_identity_type(full_identity_name.clone())?;
                        let permissions = self.get_permissions(full_identity_name.clone())?;

                        let identity = StandardIdentity::new(
                            full_identity_name,
                            None,
                            node_encryption_public_key,
                            node_signature_public_key,
                            Some(subidentity_encryption_public_key),
                            Some(subidentity_signature_public_key),
                            identity_type,
                            permissions,
                        );
                        result.push(identity);
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    pub fn get_all_profiles_and_devices(&self, my_node_identity: ShinkaiName) -> Result<Vec<Identity>, ShinkaiDBError> {
        let my_node_identity_name = my_node_identity.get_node_name_string();
        // Use Topic::NodeAndUsers for profiles and devices related information with specific prefixes
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        let (node_encryption_public_key, node_signature_public_key) =
            self.get_local_node_keys(my_node_identity.clone())?;

        let mut result: Vec<Identity> = Vec::new();
        let iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);

        for item in iter {
            match item {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    // Filter out profiles based on prefix
                    if key_str.starts_with("identity_key_of_") {
                        let profile_name = key_str.trim_start_matches("identity_key_of_");
                        let full_identity_name = ShinkaiName::from_node_and_profile_names(
                            my_node_identity_name.clone(),
                            profile_name.to_string(),
                        )?;
                        let subidentity_signature_public_key =
                            string_to_signature_public_key(&String::from_utf8(value.to_vec()).unwrap())?;
                        let subidentity_encryption_public_key =
                            self.get_subidentity_encryption_public_key(full_identity_name.clone())?;
                        let identity_type = self.get_identity_type(full_identity_name.clone())?;
                        let permissions = self.get_permissions(full_identity_name.clone())?;

                        let identity = Identity::Standard(StandardIdentity::new(
                            full_identity_name,
                            None,
                            node_encryption_public_key,
                            node_signature_public_key,
                            Some(subidentity_encryption_public_key),
                            Some(subidentity_signature_public_key),
                            identity_type,
                            permissions,
                        ));

                        result.push(identity);
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Iterate over devices
        let device_iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);
        for device_item in device_iter {
            match device_item {
                Ok((device_key, _device_value)) => {
                    let device_key_str = String::from_utf8(device_key.to_vec()).unwrap();
                    // Filter out devices based on prefix
                    if device_key_str.starts_with("device_identity_key_of_") {
                        let device_name = device_key_str.trim_start_matches("device_identity_key_of_");
                        let full_name_string = format!("{}/{}", my_node_identity_name, device_name);
                        let device_shinkai_name = ShinkaiName::new(full_name_string)?;
                        let device_identity = self.get_device(device_shinkai_name)?;
                        result.push(Identity::Device(device_identity));
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(result)
    }

    pub fn insert_profile(&self, identity: StandardIdentity) -> Result<(), ShinkaiDBError> {
        let profile_name =
            identity
                .full_identity_name
                .get_profile_name_string()
                .ok_or(ShinkaiDBError::InvalidIdentityName(
                    identity.full_identity_name.to_string(),
                ))?;

        // Use Topic::NodeAndUsers with specific prefixes for inserting profile information
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Check that the full identity name doesn't exist in the column family with specific prefixes
        if self
            .db
            .get_cf(
                cf_node_and_users,
                format!("identity_key_of_{}", profile_name).as_bytes(),
            )?
            .is_some()
            || self
                .db
                .get_cf(
                    cf_node_and_users,
                    format!("encryption_key_of_{}", profile_name).as_bytes(),
                )?
                .is_some()
            || self
                .db
                .get_cf(
                    cf_node_and_users,
                    format!("identity_type_of_{}", profile_name).as_bytes(),
                )?
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
            .unwrap_or_default();
        let sub_encryption_public_key = identity
            .profile_encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_default();

        // Put the identity details into the column family with specific prefixes
        batch.put_cf(
            cf_node_and_users,
            format!("identity_key_of_{}", profile_name).as_bytes(),
            sub_identity_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_node_and_users,
            format!("encryption_key_of_{}", profile_name).as_bytes(),
            sub_encryption_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_node_and_users,
            format!("identity_type_of_{}", profile_name).as_bytes(),
            identity.identity_type.to_string().as_bytes(),
        );

        batch.put_cf(
            cf_node_and_users,
            format!("permissions_of_{}", profile_name).as_bytes(),
            identity.permission_type.to_string().as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!("Inserted profile: {}", profile_name).as_str(),
        );

        Ok(())
    }

    pub fn does_identity_exists(&self, profile: &ShinkaiName) -> Result<bool, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .clone()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile.full_name.to_string()))?;

        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let profile_identity_key = format!("identity_key_of_{}", profile_name);

        Ok(self.db.get_cf(cf_node, profile_identity_key.as_bytes())?.is_some())
    }

    pub fn get_profile_permission(&self, profile_name: ShinkaiName) -> Result<IdentityPermissions, ShinkaiDBError> {
        let profile_name = profile_name
            .clone()
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile_name.to_string()))?;
        // Use Topic::NodeAndUsers with specific prefixes to access profile permissions
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Attempt to get the permission value for the profile name with the specific prefix
        let profile_permission_key = format!("permissions_of_{}", profile_name);
        match self.db.get_cf(cf_node_and_users, profile_permission_key.as_bytes())? {
            Some(value) => {
                // Convert the byte value into a string, and then try to parse it into IdentityPermissions
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

    pub fn get_device_permission(&self, device_name: ShinkaiName) -> Result<IdentityPermissions, ShinkaiDBError> {
        // Extract the device name from the ShinkaiName
        let device_name = device_name
            .get_fullname_string_without_node_name()
            .ok_or(ShinkaiDBError::InvalidIdentityName(device_name.to_string()))?;

        // Use Topic::NodeAndUsers with specific prefixes to access device permissions
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Attempt to get the permission value for the device name with the specific prefix
        let device_permission_key = format!("device_permissions_of_{}", device_name);
        match self.db.get_cf(cf_node_and_users, device_permission_key.as_bytes())? {
            Some(value) => {
                // Convert the byte value into a string, and then try to parse it into IdentityPermissions
                let permission_str = std::str::from_utf8(&value).map_err(|_| {
                    ShinkaiDBError::InvalidPermissionType(format!("Invalid permission type: {:?}", value))
                })?;
                IdentityPermissions::from_str(permission_str).ok_or(ShinkaiDBError::InvalidPermissionType(format!(
                    "Invalid permission type: {:?}",
                    value
                )))
            }
            None => Err(ShinkaiDBError::PermissionNotFound(format!(
                "No permission found for device: {}",
                device_name
            ))),
        }
    }

    pub fn debug_print_all_keys_for_profiles_identity_key(&self) {
        // Use Topic::NodeAndUsers for profiles related information with specific prefix
        let cf_node_and_users = match self.db.cf_handle(Topic::NodeAndUsers.as_str()) {
            Some(handle) => handle,
            None => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    "Failed to get column family handle for NodeAndUsers",
                );
                return;
            }
        };

        // Create an iterator for the column family
        let iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);

        // Iterate over the keys in the column family and print them
        for item in iter {
            match item {
                Ok((key, _value)) => {
                    // Convert the key bytes to a string for display purposes
                    let key_str = String::from_utf8(key.to_vec()).unwrap();

                    // Check if the key starts with the specific prefixes for profile identity keys
                    if key_str.starts_with("identity_key_of_") {
                        shinkai_log(
                            ShinkaiLogOption::Identity,
                            ShinkaiLogLevel::Debug,
                            format!("print_all_keys_for_profiles_identity_key {}", key_str).as_str(),
                        );
                        // Assuming the need to print all devices for a profile, extract the profile name from the key
                        let profile_name = key_str.trim_start_matches("identity_key_of_");
                        self.print_all_devices_for_profile(profile_name);
                    }
                }
                Err(e) => {
                    // Optionally handle the error, e.g., print it out
                    shinkai_log(
                        ShinkaiLogOption::Identity,
                        ShinkaiLogLevel::Error,
                        format!("Error reading from database: {}", e).as_str(),
                    );
                }
            }
        }
    }

    pub fn print_all_devices_for_profile(&self, profile_name: &str) {
        // Use Topic::NodeAndUsers for devices related information with specific prefix
        let cf_node_and_users = match self.db.cf_handle(Topic::NodeAndUsers.as_str()) {
            Some(handle) => handle,
            None => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    "Failed to get column family handle for NodeAndUsers",
                );
                return;
            }
        };

        // Create an iterator for the column family
        let iter = self.db.iterator_cf(cf_node_and_users, rocksdb::IteratorMode::Start);

        // Iterate over the keys in the column family and print them
        for item in iter {
            match item {
                Ok((key, _value)) => {
                    // Convert the key bytes to a string
                    let key_str = String::from_utf8(key.to_vec()).unwrap();

                    // Check if the key (device identity name) contains the profile name with the specific prefix
                    if key_str.starts_with(&format!("device_identity_key_of_{}", profile_name)) {
                        shinkai_log(
                            ShinkaiLogOption::Identity,
                            ShinkaiLogLevel::Debug,
                            format!("print_all_devices_for_profile {}", key_str).as_str(),
                        );
                    }
                }
                Err(e) => {
                    // Optionally handle the error, e.g., print it out
                    shinkai_log(
                        ShinkaiLogOption::Identity,
                        ShinkaiLogLevel::Error,
                        format!("Error reading from database: {}", e).as_str(),
                    );
                }
            }
        }
    }

    pub fn add_device_to_profile(&self, device: DeviceIdentity) -> Result<(), ShinkaiDBError> {
        // Get the profile name from the device identity name
        let profile_name = match device.full_identity_name.get_profile_name_string() {
            Some(name) => name,
            None => {
                return Err(ShinkaiDBError::InvalidIdentityName(
                    device.full_identity_name.to_string(),
                ))
            }
        };

        // Use Topic::NodeAndUsers with specific prefixes to check if the profile exists
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let profile_key_prefix = format!("identity_key_of_{}", profile_name);
        if self
            .db
            .get_cf(cf_node_and_users, profile_key_prefix.as_bytes())?
            .is_none()
        {
            return Err(ShinkaiDBError::ProfileNotFound(profile_name.to_string()));
        }

        // Check that the full device identity name doesn't already exist using a specific prefix
        let shinkai_device_name = ShinkaiName::new(device.full_identity_name.to_string())?;
        let device_name = shinkai_device_name
            .get_fullname_string_without_node_name()
            .ok_or(ShinkaiDBError::InvalidIdentityName(shinkai_device_name.to_string()))?;
        let device_key_prefix = format!("device_identity_key_of_{}", device_name);
        if self
            .db
            .get_cf(cf_node_and_users, device_key_prefix.as_bytes())?
            .is_some()
        {
            return Err(ShinkaiDBError::DeviceIdentityAlreadyExists(
                device.full_identity_name.to_string(),
            ));
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Convert the public keys to strings
        let device_signature_public_key = signature_public_key_to_string_ref(&device.device_signature_public_key);
        let device_encryption_public_key = encryption_public_key_to_string_ref(&device.device_encryption_public_key);

        // Add the device information to the batch using specific prefixes
        batch.put_cf(
            cf_node_and_users,
            format!("device_identity_key_of_{}", device_name).as_bytes(),
            device_signature_public_key.as_bytes(),
        );
        batch.put_cf(
            cf_node_and_users,
            format!("device_encryption_key_of_{}", device_name).as_bytes(),
            device_encryption_public_key.as_bytes(),
        );

        // Add the device permission to the batch using a specific prefix
        let permission_str = device.permission_type.to_string();
        batch.put_cf(
            cf_node_and_users,
            format!("device_permissions_of_{}", device_name).as_bytes(),
            permission_str.as_bytes(),
        );

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_profile(&self, name: &str) -> Result<(), ShinkaiDBError> {
        // Use Topic::NodeAndUsers with specific prefixes for each aspect of the profile
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Prefixes for checking existence and deletion
        let identity_key_prefix = format!("identity_key_of_{}", name);
        let encryption_key_prefix = format!("encryption_key_of_{}", name);
        let permission_key_prefix = format!("permissions_of_{}", name);
        let identity_type_key_prefix = format!("identity_type_of_{}", name);

        // Check that the profile name exists for each aspect
        if self
            .db
            .get_cf(cf_node_and_users, identity_key_prefix.as_bytes())?
            .is_none()
            || self
                .db
                .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())?
                .is_none()
            || self
                .db
                .get_cf(cf_node_and_users, permission_key_prefix.as_bytes())?
                .is_none()
            || self
                .db
                .get_cf(cf_node_and_users, identity_type_key_prefix.as_bytes())?
                .is_none()
        {
            return Err(ShinkaiDBError::ProfileNameNonExistent(name.to_string()));
        }

        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();

        // Delete each aspect of the profile using the specific prefixes
        batch.delete_cf(cf_node_and_users, identity_key_prefix.as_bytes());
        batch.delete_cf(cf_node_and_users, encryption_key_prefix.as_bytes());
        batch.delete_cf(cf_node_and_users, permission_key_prefix.as_bytes());
        batch.delete_cf(cf_node_and_users, identity_type_key_prefix.as_bytes());

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_profile(&self, full_identity_name: ShinkaiName) -> Result<Option<StandardIdentity>, ShinkaiDBError> {
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        // Use Topic::NodeAndUsers for all profile related information with specific prefixes
        let cf_node_and_users = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("NodeAndUsers".to_string()))?;

        let identity_key_prefix = format!("identity_key_of_{}", profile_name);

        // Check if the profile exists
        let profile_exists = match self.db.get_cf(cf_node_and_users, identity_key_prefix.as_bytes()) {
            Ok(Some(_)) => true,
            _ => false,
        };

        if !profile_exists {
            return Ok(None);
            // return Err(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()));
        }

        let encryption_key_prefix = format!("encryption_key_of_{}", profile_name);
        let identity_type_key_prefix = format!("identity_type_of_{}", profile_name);
        let permission_key_prefix = format!("permissions_of_{}", profile_name);

        let identity_public_key_bytes = match self.db.get_cf(cf_node_and_users, identity_key_prefix.as_bytes())? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let encryption_public_key_bytes = self
            .db
            .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;
        let identity_type_bytes = self
            .db
            .get_cf(cf_node_and_users, identity_type_key_prefix.as_bytes())?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;
        let permission_type_bytes = self
            .db
            .get_cf(cf_node_and_users, permission_key_prefix.as_bytes())?
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

        let (node_encryption_public_key, node_signature_public_key) =
            self.get_local_node_keys(full_identity_name.clone())?;

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

    pub fn get_device(&self, full_identity_name: ShinkaiName) -> Result<DeviceIdentity, ShinkaiDBError> {
        let device_name = full_identity_name
            .get_fullname_string_without_node_name()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        // Use Topic::NodeAndUsers for device related information with specific prefixes
        let cf_node_and_users = self
            .db
            .cf_handle(Topic::NodeAndUsers.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("NodeAndUsers".to_string()))?;

        let device_signature_public_key_bytes = match self.db.get_cf(
            cf_node_and_users,
            format!("device_identity_key_of_{}", device_name).as_bytes(),
        )? {
            Some(bytes) => bytes,
            None => return Err(ShinkaiDBError::DeviceNameNonExistent(device_name.to_string())),
        };

        let device_encryption_public_key_bytes = match self.db.get_cf(
            cf_node_and_users,
            format!("device_encryption_key_of_{}", device_name).as_bytes(),
        )? {
            Some(bytes) => bytes,
            None => return Err(ShinkaiDBError::DeviceNameNonExistent(device_name.to_string())),
        };

        let permission_type_bytes = match self.db.get_cf(
            cf_node_and_users,
            format!("device_permissions_of_{}", device_name).as_bytes(),
        )? {
            Some(value) => value,
            None => return Err(ShinkaiDBError::DeviceNameNonExistent(device_name.to_string())),
        };

        let device_signature_public_key_str = String::from_utf8(device_signature_public_key_bytes.to_vec())
            .map_err(|_| ShinkaiDBError::Utf8ConversionError)?;

        let device_encryption_public_key_str = String::from_utf8(device_encryption_public_key_bytes.to_vec())
            .map_err(|_| ShinkaiDBError::Utf8ConversionError)?;

        let permission_type_str =
            String::from_utf8(permission_type_bytes.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;

        let device_signature_public_key = string_to_signature_public_key(&device_signature_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

        let device_encryption_public_key = string_to_encryption_public_key(&device_encryption_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

        let permission_type =
            IdentityPermissions::from_str(&permission_type_str).ok_or(ShinkaiDBError::InvalidPermissionsType)?;

        let (node_encryption_public_key, node_signature_public_key) =
            self.get_local_node_keys(full_identity_name.clone())?;

        // Extract profile_name from full_identity_name
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        let profile_encryption_public_key_bytes = self
            .db
            .get_cf(
                cf_node_and_users,
                format!("encryption_key_of_{}", profile_name).as_bytes(),
            )?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;
        let profile_signature_public_key_bytes = self
            .db
            .get_cf(
                cf_node_and_users,
                format!("identity_key_of_{}", profile_name).as_bytes(),
            )?
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(profile_name.to_string()))?;

        let profile_encryption_public_key_str = String::from_utf8(profile_encryption_public_key_bytes.to_vec())
            .map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
        let profile_signature_public_key_str = String::from_utf8(profile_signature_public_key_bytes.to_vec())
            .map_err(|_| ShinkaiDBError::Utf8ConversionError)?;

        let profile_encryption_public_key = string_to_encryption_public_key(&profile_encryption_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

        let profile_signature_public_key = string_to_signature_public_key(&profile_signature_public_key_str)
            .map_err(|_| ShinkaiDBError::PublicKeyParseError)?;

        Ok(DeviceIdentity {
            full_identity_name,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key,
            profile_signature_public_key,
            device_encryption_public_key,
            device_signature_public_key,
            permission_type,
        })
    }

    pub fn get_subidentity_encryption_public_key(
        &self,
        full_identity_name: ShinkaiName,
    ) -> Result<EncryptionPublicKey, ShinkaiDBError> {
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        // Use Topic::NodeAndUsers with a special prefix for encryption keys
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let encryption_key_prefix = format!("encryption_key_of_{}", profile_name);

        match self.db.get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())? {
            Some(value) => {
                let key_string = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
                string_to_encryption_public_key(&key_string).map_err(|_| ShinkaiDBError::PublicKeyParseError)
            }
            None => Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name.to_string())),
        }
    }

    pub fn get_identity_type(&self, full_identity_name: ShinkaiName) -> Result<StandardIdentityType, ShinkaiDBError> {
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        // Use Topic::NodeAndUsers with a special prefix for identity type
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let identity_type_key_prefix = format!("identity_type_of_{}", profile_name);

        match self.db.get_cf(cf_node_and_users, identity_type_key_prefix.as_bytes())? {
            Some(value) => {
                let identity_type_str = String::from_utf8(value.to_vec()).unwrap();
                StandardIdentityType::to_enum(&identity_type_str).ok_or(ShinkaiDBError::InvalidIdentityType(format!(
                    "Invalid identity type for: {}",
                    identity_type_str
                )))
            }
            None => Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name.to_string())),
        }
    }

    pub fn get_permissions(&self, full_identity_name: ShinkaiName) -> Result<IdentityPermissions, ShinkaiDBError> {
        let profile_name = full_identity_name
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidIdentityName(full_identity_name.to_string()))?;

        // Use Topic::NodeAndUsers with a special prefix for permissions
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let permissions_key_prefix = format!("permissions_of_{}", profile_name);

        match self.db.get_cf(cf_node_and_users, permissions_key_prefix.as_bytes())? {
            Some(value) => {
                let permissions_str = String::from_utf8(value.to_vec()).unwrap();
                IdentityPermissions::from_str(&permissions_str).ok_or(ShinkaiDBError::InvalidPermissionsType)
            }
            None => Err(ShinkaiDBError::ProfileNameNonExistent(full_identity_name.to_string())),
        }
    }
}
