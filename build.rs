// build.rs
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::Path;

fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();

    let url = if cfg!(target_os = "macos") {
        "https://github.com/go-skynet/LocalAI/releases/download/v1.20.1/local-ai-avx2-Darwin-x86_64"
    } else if cfg!(target_os = "linux") {
        "https://github.com/go-skynet/LocalAI/releases/download/v1.20.1/local-ai-avx2-Linux-x86_64"
    } else {
        panic!("Unsupported OS");
    };
    let output_filename = "local-ai";

    download_file(url, output_filename, output_filename);

    let model_url = "https://huggingface.co/skeskinen/ggml/resolve/main/all-MiniLM-L12-v2/ggml-model-q4_1.bin";
    let model_filename = "models/all-MiniLM-L12-v2.bin";

    download_file(model_url, model_filename, model_filename);
}

fn download_file(url: &str, filename: &str, output_filename: &str) {
    // Check if the file exists
    if !Path::new(output_filename).exists() {
        // File does not exist, download it
        println!("Downloading {}...", filename);

        let response = reqwest::blocking::get(url);
        match response {
            Ok(mut resp) => {
                if resp.status().is_success() {
                    // Ensure the parent directory exists
                    if let Some(parent) = Path::new(output_filename).parent() {
                        create_dir_all(parent).expect("Failed to create directory");
                    }

                    let mut out = File::create(output_filename).expect("Failed to create file");
                    copy(&mut resp, &mut out).expect("Failed to copy content");
                    println!("{} downloaded successfully.", filename);
                } else {
                    println!("Failed to download {}: {}", filename, resp.status());
                }
            }
            Err(e) => {
                println!("Failed to download {}: {}", filename, e);
            }
        }
    } else {
        println!("{} already exists.", filename);
    }
}
