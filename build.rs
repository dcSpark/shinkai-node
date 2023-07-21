// build.rs
use std::fs::File;
use std::io::copy;
use std::path::Path;

fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();

    let filename = "pythia-160m-q4_0.bin";
    let url = "https://huggingface.co/rustformers/pythia-ggml/resolve/main/pythia-160m-q4_0.bin";

    // Check if the file exists
    if !Path::new(filename).exists() {
        // File does not exist, download it
        println!("Downloading Embeddings Model...");

        let response = reqwest::blocking::get(url);
        match response {
            Ok(mut resp) => {
                if resp.status().is_success() {
                    let mut out = File::create(filename).expect("Failed to create file");
                    copy(&mut resp, &mut out).expect("Failed to copy content");
                    println!("Model downloaded successfully.");
                } else {
                    println!("Failed to download the model: {}", resp.status());
                }
            }
            Err(e) => {
                println!("Failed to download the model: {}", e);
            }
        }
    } else {
        println!("File already exists.");
    }
}
