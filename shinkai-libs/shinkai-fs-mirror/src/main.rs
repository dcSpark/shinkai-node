use clap::{App, Arg};
use dotenv::dotenv;
use shinkai_fs_mirror::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;
use shinkai_fs_mirror::synchronizer::{FilesystemSynchronizer, SyncInterval};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    dotenv().ok(); // Load .env file if exists

    let matches = App::new("Shinkai FS Mirror")
        .version("1.0")
        .author("Nico Arqueros <nico@shinkai.com>")
        .about("Synchronizes filesystem changes with Shinkai")
        .arg(
            Arg::with_name("encrypted_file_path")
                .short('f')
                .long("file")
                .value_name("FILE")
                .help("Sets the path to the encrypted file containing keys")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("passphrase")
                .short('p')
                .long("pass")
                .value_name("PASSPHRASE")
                .help("Passphrase for the encrypted file (can also be set via ENV)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("destination_path")
                .short('d')
                .long("dest")
                .value_name("DESTINATION_PATH")
                .help("Destination path for the synchronization")
                .takes_value(true)
                .required(true), // Marking this argument as required
        )
        .get_matches();

    let encrypted_file_path = env::var("ENCRYPTED_FILE_PATH")
        .ok()
        .or_else(|| matches.value_of("encrypted_file_path").map(String::from))
        .expect("encrypted_file_path is required");

    let passphrase = matches
        .value_of("passphrase")
        .map(String::from)
        .or_else(|| env::var("PASSPHRASE").ok());

    let destination_path = env::var("DESTINATION_PATH")
        .map(PathBuf::from)
        .or_else(|_| {
            matches
                .value_of("destination_path")
                .map(PathBuf::from)
                .ok_or_else(|| "Destination path is required".to_string())
        })
        .expect("Required");

    // Example usage, adjust according to your actual needs
    let folder_to_watch = PathBuf::from("/path/to/watch");
    let db_path = "path/to/db".to_string();
    let sync_interval = SyncInterval::Immediate; // Or whatever logic you want to determine this

    // Example of creating a FilesystemSynchronizer
    let shinkai_manager = ShinkaiManagerForSync::initialize_from_encrypted_file_path(
        Path::new(&encrypted_file_path),
        passphrase.as_deref().unwrap_or(""),
    )
    .expect("Failed to initialize ShinkaiManagerForSync");

    let synchronizer = FilesystemSynchronizer::new(
        shinkai_manager,
        folder_to_watch,
        destination_path,
        db_path,
        sync_interval,
    )
    .await
    .expect("Failed to create FilesystemSynchronizer");

    println!("{:?}", synchronizer);
}