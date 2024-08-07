use async_channel::Sender;
use qrcode::{Color, QrCode};
use serde::Serialize;
use shinkai_message_primitives::{shinkai_utils::encryption::encryption_public_key_to_string, shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType}};

use crate::network::node_commands::NodeCommand;

use super::{keys::NodeKeys, environment::NodeEnvironment};

#[derive(Serialize, Clone)]
pub struct QRSetupData {
    pub registration_code: String,
    pub profile: String,
    pub identity_type: String,
    pub permission_type: String,
    pub node_address: String,
    pub shinkai_identity: String,
    pub node_encryption_pk: String,
    pub node_signature_pk: String,
}

pub async fn generate_qr_codes(
    node_commands_sender: &Sender<NodeCommand>,
    node_env: &NodeEnvironment,
    node_keys: &NodeKeys,
    global_identity_name: &str,
    identity_public_key_string: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let node_address = {
        let address_str = node_env.api_listen_address.to_string();
        if !address_str.starts_with("http://") {
            format!("http://{}", address_str)
        } else {
            address_str
        }
    };

    // Generate QR codes for devices
    for i in 0..node_env.starting_num_qr_devices {
        let (res1_registration_sender, res1_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::LocalCreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Device("main".to_string()),
                res: res1_registration_sender,
            })
            .await?;
        let node_registration_code = res1_registraton_receiver.recv().await?;

        let qr_data = QRSetupData {
            registration_code: node_registration_code,
            profile: "main".to_string(),
            identity_type: "device".to_string(),
            permission_type: "admin".to_string(),
            node_address: node_address.clone(),
            shinkai_identity: global_identity_name.to_string(),
            node_encryption_pk: encryption_public_key_to_string(node_keys.encryption_public_key),
            node_signature_pk: identity_public_key_string.to_string(),
        };

        let qr_code_name = format!("qr_code_device_{}", i);
        save_qr_data_to_local_image(qr_data.clone(), qr_code_name.clone());
        print_qr_data_to_console(qr_data.clone(), "device");
        display_qr(&qr_data);
    }

    // TODO: Decide if we actually need this
    // // Generate QR codes for profiles
    // for i in 0..node_env.starting_num_qr_profiles {
    //     let (res1_registration_sender, res1_registraton_receiver) = async_channel::bounded(1);
    //     node_commands_sender
    //         .send(NodeCommand::LocalCreateRegistrationCode {
    //             permissions: IdentityPermissions::Admin,
    //             code_type: RegistrationCodeType::Profile,
    //             res: res1_registration_sender,
    //         })
    //         .await?;
    //     let node_registration_code = res1_registraton_receiver.recv().await?;

    //     let qr_data = QRSetupData {
    //         registration_code: node_registration_code,
    //         profile: "".to_string(),
    //         identity_type: "profile".to_string(),
    //         permission_type: "admin".to_string(),
    //         node_address: node_address.clone(),
    //         shinkai_identity: global_identity_name.to_string(),
    //         node_encryption_pk: encryption_public_key_to_string(node_keys.encryption_public_key.clone()),
    //         node_signature_pk: identity_public_key_string.to_string(),
    //     };

    //     let qr_code_name = format!("qr_code_profile_{}", i);
    //     save_qr_data_to_local_image(qr_data.clone(), qr_code_name.clone());
    //     print_qr_data_to_console(qr_data.clone(), "profile");
    //     display_qr(&qr_data);
    // }

    Ok(())
}

pub fn save_qr_data_to_local_image(qr_data: QRSetupData, name: String) {
    // Serialize QR data to JSON
    let qr_json_string = serde_json::to_string(&qr_data).expect("Failed to serialize QR data to JSON");

    // Generate and save QR code as an image
    let qr_code = QrCode::new(qr_json_string.as_bytes()).unwrap();
    let image = qr_code.render::<image::Luma<u8>>().build();
    image.save(format!("{}.png", name)).unwrap();
}

pub fn print_qr_data_to_console(qr_data: QRSetupData, node_profile: &str) {
    // Print qr_data to console in a beautiful way
    println!("Please scan the QR code below with your phone to register this device:");
    println!("---------------------------------------------------------------");
    println!("Node registration code: {}", qr_data.registration_code);
    println!("Node profile: {}", node_profile);
    println!("Node identity type: {}", qr_data.identity_type);
    println!("Node permission type: admin");
    println!("Node address: {}", qr_data.node_address);
    println!("Node Shinkai identity: {}", qr_data.shinkai_identity);
    println!("Node encryption pk: {}", qr_data.node_encryption_pk);
    println!("Node signature pk: {}", qr_data.node_signature_pk);
    println!("---------------------------------------------------------------");
}

pub fn display_qr<T: Serialize>(data: &T) {
    let qr_json_string = serde_json::to_string(data).expect("Failed to serialize data to JSON");

    // Generate QR code from serialized data
    let qr_code = QrCode::new(qr_json_string.as_bytes()).unwrap();

    let border = 2;
    let colors = qr_code.to_colors();
    let size = (colors.len() as f64).sqrt() as usize;

    for y in (-border..size as isize + border).step_by(2) {
        for x in -border..size as isize + border {
            let get_color = |x: isize, y: isize| -> bool {
                if x >= 0 && y >= 0 && x < size as isize && y < size as isize {
                    colors[y as usize * size + x as usize] == Color::Dark
                } else {
                    false
                }
            };

            let top_module = get_color(x, y);
            let bottom_module = get_color(x, y + 1);

            match (top_module, bottom_module) {
                (true, true) => print!("█"),
                (true, false) => print!("▀"),
                (false, true) => print!("▄"),
                _ => print!(" "),
            }
        }
        println!();
    }
}
