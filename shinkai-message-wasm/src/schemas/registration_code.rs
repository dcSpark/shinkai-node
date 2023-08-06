use serde::{Serialize, Deserialize};

// Note: there is another declaration of RegistrationCode
// but re-using it means to import a lot of chained things
// so we are making a dummy slightly diff compatible clone here
#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
    pub identity_type: String,
    pub permission_type: String,
}