use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct ShinkaiDeviceKeys {
    pub my_device_encryption_pk: String,
    pub my_device_encryption_sk: String,
    pub my_device_identity_pk: String,
    pub my_device_identity_sk: String,
    pub profile_encryption_pk: String,
    pub profile_encryption_sk: String,
    pub profile_identity_pk: String,
    pub profile_identity_sk: String,
    pub profile: String,
    pub identity_type: Option<String>,
    pub permission_type: String,
    pub shinkai_identity: String,
    pub registration_code: Option<String>,
    pub node_encryption_pk: String,
    pub node_address: String,
    pub registration_name: String,
    pub node_signature_pk: String,
}