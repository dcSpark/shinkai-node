use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct QRSetupData {
    pub registration_code: String,
    pub profile: String,
    pub registration_type: String,
    pub node_ip: String,
    pub node_port: String,
    pub shinkai_identity: String,
    pub node_encryption_pk: String,
    pub node_signature_pk: String,
}
