use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
    pub permission_type: String,
}