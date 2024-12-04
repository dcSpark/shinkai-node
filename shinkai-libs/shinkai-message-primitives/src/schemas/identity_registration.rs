use crate::shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType};

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
