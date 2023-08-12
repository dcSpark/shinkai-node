use qrcode::{Color, QrCode};
use serde::Serialize;

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

pub fn save_qr_data_to_local_image(qr_data: QRSetupData, name: String) {
    // Serialize QR data to JSON
    let qr_json_string = serde_json::to_string(&qr_data).expect("Failed to serialize QR data to JSON");

    // Generate and save QR code as an image
    let qr_code = QrCode::new(qr_json_string.as_bytes()).unwrap();
    let image = qr_code.render::<image::Luma<u8>>().build();
    image.save(format!("{}.png", name)).unwrap();
}

pub fn print_qr_data_to_console(qr_data: QRSetupData) {
    // Print qr_data to console in a beautiful way
    println!("Please scan the QR code below with your phone to register this device:");
    println!("---------------------------------------------------------------");
    println!("Node registration code: {}", qr_data.registration_code);
    println!("Node profile: main");
    println!("Node identity type: device");
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
