// build.rs
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();

    /// Auto-fetching embeddings model if not found in local folder
    let filename = "pythia-160m-q4_0.bin";
    let url = "https://huggingface.co/rustformers/pythia-ggml/resolve/main/pythia-160m-q4_0.bin";

    // Check if the file exists
    if !Path::new(filename).exists() {
        // File does not exist, download it
        println!("Downloading Pythia Embeddings Model...");
        let output = Command::new("curl")
            .arg("-o")
            .arg(filename)
            .arg(url)
            .output()
            .expect("Failed to download the model.");

        // Check the download status
        if output.status.success() {
            println!("Model downloaded successfully.");
        } else {
            let error_message = String::from_utf8_lossy(&output.stderr);
            println!("Failed to download the model: {}", error_message);
        }
    } else {
        println!("File already exists.");
    }
}
