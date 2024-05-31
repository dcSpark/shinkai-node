use clap::{App, Arg};
use dotenv::dotenv;
use shinkai_fs_mirror::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;
use shinkai_fs_mirror::synchronizer::{FilesystemSynchronizer, SyncInterval};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[tokio::main]
async fn main() {
    dotenv().ok(); // Load .env file if exists

    let matches = App::new("Shinkai FS Mirror")
        .version(env!("CARGO_PKG_VERSION"))
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
        .arg(
            Arg::with_name("folder_to_watch")
                .short('w')
                .long("watch")
                .value_name("FOLDER_TO_WATCH")
                .help("Folder path to watch for changes")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("db_path")
                .short('b')
                .long("db")
                .value_name("DB_PATH")
                .help("Database path for storing synchronization data")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("sync_interval")
                .short('i')
                .long("interval")
                .value_name("SYNC_INTERVAL")
                .help("Sync interval (immediate, timed:<seconds>, none)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("node_address")
                .short('n')
                .long("node")
                .value_name("NODE_ADDRESS")
                .help("Node address for synchronization")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("should_mirror_deletes")
                .short('r')
                .long("should-mirror-deletes")
                .value_name("SHOULD_MIRROR_DELETES")
                .help("If set, files deleted locally will also be removed remotely")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("upload_timeout")
                .short('u')
                .long("upload-timeout")
                .value_name("UPLOAD_TIMEOUT")
                .help("Upload timeout in seconds (optional)")
                .takes_value(true),
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

    let folder_to_watch = matches
        .value_of("folder_to_watch")
        .map(PathBuf::from)
        .or_else(|| env::var("FOLDER_TO_WATCH").ok().map(PathBuf::from))
        .expect("Folder to watch is required");

    let db_path = matches
        .value_of("db_path")
        .map(String::from)
        .or_else(|| env::var("DB_PATH").ok())
        .unwrap_or_else(|| "mirror_db".to_string());

    let sync_interval_str = matches
        .value_of("sync_interval")
        .map(String::from)
        .or_else(|| env::var("SYNC_INTERVAL").ok())
        .unwrap_or_else(|| "immediate".to_string()); // Default value

    let sync_interval = parse_sync_interval(&sync_interval_str).expect("Failed to parse sync interval");

    let node_address = matches
        .value_of("node_address")
        .map(String::from)
        .or_else(|| env::var("NODE_ADDRESS").ok());

    let should_mirror_deletes = matches.is_present("should_mirror_deletes");

    let upload_timeout = matches
        .value_of("upload_timeout")
        .map(|s| s.parse::<u64>().expect("Failed to parse upload timeout"))
        .map(Duration::from_secs)
        .or_else(|| {
            env::var("UPLOAD_TIMEOUT")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .map(Duration::from_secs)
        });

    let mut shinkai_manager = ShinkaiManagerForSync::initialize_from_encrypted_file_path(
        Path::new(&encrypted_file_path),
        passphrase.as_deref().unwrap_or(""),
    )
    .expect("Failed to initialize ShinkaiManagerForSync");

    if node_address.is_some() {
        shinkai_manager.node_address = node_address.unwrap();
    }

    let synchronizer = FilesystemSynchronizer::new(
        shinkai_manager,
        folder_to_watch,
        destination_path,
        db_path,
        sync_interval,
        should_mirror_deletes,
        upload_timeout,
    )
    .await
    .expect("Failed to create FilesystemSynchronizer");

    println!("{:?}", synchronizer);

    println!("Running. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Exiting.");
}

fn parse_sync_interval(input: &str) -> Result<SyncInterval, &'static str> {
    match input.to_lowercase().as_str() {
        "immediate" => Ok(SyncInterval::Immediate),
        "none" => Ok(SyncInterval::None),
        s if s.starts_with("timed:") => {
            let seconds_str = &s[6..];
            seconds_str
                .parse::<u64>()
                .map(Duration::from_secs)
                .map(SyncInterval::Timed)
                .map_err(|_| "Failed to parse duration for timed sync interval")
        }
        _ => Err("Invalid sync interval format"),
    }
}
