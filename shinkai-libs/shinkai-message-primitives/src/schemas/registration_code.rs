use serde::{Serialize, Deserialize};

// Note: there is another declaration of RegistrationCode
// but re-using it means to import a lot of chained things
// so we are making a dummy slightly diff compatible clone here
#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCodeSimple {
    pub code: String,
    pub registration_name: String,
    pub device_identity_pk: String,
    pub device_encryption_pk: String,
    pub profile_identity_pk: String,
    pub profile_encryption_pk: String,
    pub identity_type: String,
    pub permission_type: String,
}