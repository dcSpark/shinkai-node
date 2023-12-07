// build.rs
use std::fs::{self, Permissions};
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Prepare toolkit executor and example toolkit to use with rust tests
    // let output = Command::new("sh")
    //     .arg("./scripts/prepare_test_toolkit_runner.sh")
    //     .output()
    //     .expect("TS test toolkit building and preparation failed");

    // Clone repo, build, and copy the Bert.cpp compiled binary server to root
    // prepare_bert_cpp();

    // Remote Embedding Generator model (used via Bert.cpp server)
    // let model_url = "https://huggingface.co/skeskinen/ggml/resolve/main/all-MiniLM-L6-v2/ggml-model-q4_1.bin";
    // let model_filename = "models/all-MiniLM-L6-v2.bin";
    // download_file(model_url, model_filename, model_filename);
}

fn prepare_bert_cpp() {
    // Try to get the "CARGO_MANIFEST_DIR" environment variable
    match env::var("CARGO_MANIFEST_DIR") {
        Ok(manifest_dir) => {
            // If successful, create the server file path
            let server_file_path = format!("{}/bert-cpp-server", manifest_dir);

            // Check if the "bert-cpp-server" file exists
            if !Path::new(&server_file_path).exists() {
                // If the file does not exist, try to run the command
                match Command::new("sh")
                    .current_dir(&manifest_dir)
                    .arg("scripts/compile_bert_cpp.sh")
                    .status()
                {
                    Ok(_) => {
                        // If successful, try to set the execute permission
                        if let Err(e) = set_execute_permission("bert-cpp-server") {
                            println!("Failed to set execute permission: {}", e);
                        }
                    }
                    Err(e) => {
                        println!("Failed to run command: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to read environment variable: {}", e);
        }
    }
}

fn set_execute_permission(path: &str) -> std::io::Result<()> {
    let permissions = Permissions::from_mode(0o755); // rwxr-xr-x
    fs::set_permissions(path, permissions)
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
