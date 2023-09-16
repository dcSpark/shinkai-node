use serde::Deserialize;
use reqwest::Error;
use shinkai_message_primitives;

#[derive(Deserialize)]
pub struct OnboardingData {
    node_address: String,
    registration_code: String,
}

#[tauri::command]
pub fn process_onboarding_data(data: OnboardingData) -> String {
    // The rest of your function here...
    // Process the data here
    // For now, let's just print the data and return a success message
    println!("Node Address: {}", data.node_address);
    println!("Registration Code: {}", data.registration_code);

    // Generate keys
    let profile_encryption_keys = shinkai_message_primitives::shinkai_utils::encryption::ephemeral_encryption_keys();
    let profile_signing_keys = shinkai_message_primitives::shinkai_utils::signatures::ephemeral_signature_keypair();

    let device_encryption_keys = shinkai_message_primitives::shinkai_utils::encryption::ephemeral_encryption_keys();
    let device_signing_keys = shinkai_message_primitives::shinkai_utils::signatures::ephemeral_signature_keypair();

    let message = shinkai_message_primitives::shinkai_utils::shinkai_message_builder::use_code_registration_for_profile(

    )



    "Data received successfully".to_string()
}
