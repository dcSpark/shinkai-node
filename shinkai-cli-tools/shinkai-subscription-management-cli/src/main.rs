use clap::{App, Arg, SubCommand};
use dotenv::dotenv;
use shinkai_message_primitives::{
    schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment},
    shinkai_message::shinkai_message_schemas::FileDestinationCredentials,
};
use shinkai_subscription_management_cli::{
    shinkai::shinkai_manager_for_subs::ShinkaiManagerForSubs, subscription_manager::SubscriptionManager,
};
use std::{env, path::Path, process};

#[tokio::main]
async fn main() {
    dotenv().ok(); // Load .env file if exists

    let matches = App::new("Shinkai Subscription Manager CLI")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Nico Arqueros <nico@shinkai.com>")
        .about("Manages subscriptions and other handy stuff for a Shinkai node")
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
        .subcommand(SubCommand::with_name("check_node_health").about("Checks the health of the node"))
        .subcommand(
            SubCommand::with_name("get_my_node_folder")
                .about("Gets a folder from the node")
                .arg(
                    Arg::with_name("path")
                        .help("The path of the folder")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("get_my_node_folder_raw")
                .about("Gets a folder from the node")
                .arg(
                    Arg::with_name("path")
                        .help("The path of the folder")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("create_folder")
                .about("Creates a new folder in the node")
                .arg(
                    Arg::with_name("folder_name")
                        .help("The name of the folder to create")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("path")
                        .help("The path where to create the folder")
                        .required(true)
                        .index(2),
                ),
        )
        .subcommand(
            SubCommand::with_name("share_folder")
                .about("Shares a folder with another node")
                .arg(
                    Arg::with_name("path")
                        .help("The path of the folder to share")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("minimum_token_delegation")
                        .long("min-token-delegation")
                        .value_name("MIN_TOKEN_DELEGATION")
                        .help("Minimum token delegation")
                        .takes_value(true)
                        .empty_values(true),
                )
                .arg(
                    Arg::with_name("minimum_time_delegated_hours")
                        .long("min-time-delegated-hours")
                        .value_name("MIN_TIME_DELEGATED_HOURS")
                        .help("Minimum time delegated in hours")
                        .takes_value(true)
                        .empty_values(true),
                )
                .arg(
                    Arg::with_name("monthly_payment")
                        .long("monthly-payment")
                        .value_name("MONTHLY_PAYMENT")
                        .help("Monthly payment option in JSON format")
                        .takes_value(true)
                        .empty_values(true),
                )
                .arg(
                    Arg::with_name("is_free")
                        .long("is-free")
                        .value_name("IS_FREE")
                        .help("Is the folder free to access")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("has_web_alternative")
                        .long("has-web-alternative")
                        .value_name("HAS_WEB_ALTERNATIVE")
                        .help("Indicates if there is a web alternative for accessing the folder")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("source")
                        .long("source")
                        .value_name("SOURCE")
                        .help("Source type for web alternative (e.g., S3, R2)")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("access_key_id")
                        .long("access-key-id")
                        .value_name("ACCESS_KEY_ID")
                        .help("Access key ID for the source")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("secret_access_key")
                        .long("secret-access-key")
                        .value_name("SECRET_ACCESS_KEY")
                        .help("Secret access key for the source")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("endpoint_uri")
                        .long("endpoint-uri")
                        .value_name("ENDPOINT_URI")
                        .help("Endpoint URI for the source")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("bucket")
                        .long("bucket")
                        .value_name("BUCKET")
                        .help("Bucket name for the source")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("folder_description")
                        .long("folder-description")
                        .value_name("FOLDER_DESCRIPTION")
                        .help("Description of the folder being shared")
                        .takes_value(true)
                        .required(false), // Not required, can be fetched from ENV or use default
                ),
        )
        .subcommand(
            SubCommand::with_name("subscribe_to_folder")
                .about("Subscribes to a folder on another node")
                .arg(
                    Arg::with_name("path")
                        .help("The path of the folder to subscribe to")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("node_name")
                        .help("The name of the node where the folder resides")
                        .required(true)
                        .index(2),
                )
                .arg(
                    Arg::with_name("profile_name")
                        .help("The profile name to use for subscription")
                        .required(true)
                        .index(3),
                )
                .arg(
                    Arg::with_name("http_preferred")
                        .long("http-preferred")
                        .value_name("HTTP_PREFERRED")
                        .help("Prefer HTTP for subscription")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::with_name("base_folder")
                        .long("base-folder")
                        .value_name("BASE_FOLDER")
                        .help("Base folder for the subscription")
                        .takes_value(true)
                        .required(false),
                ),
        )
        .subcommand(SubCommand::with_name("my_subscriptions").about("Lists all subscriptions"))
        .subcommand(SubCommand::with_name("my_shared_folders").about("Lists all folders shared by the node"))
        .subcommand(
            SubCommand::with_name("available_shared_items")
                .about("Lists available shared items from a specific node and profile")
                .arg(
                    Arg::with_name("path")
                        .help("The path to list shared items from")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("node_name")
                        .help("The name of the node to list shared items from")
                        .required(true)
                        .index(2),
                )
                .arg(
                    Arg::with_name("profile_name")
                        .help("The profile name to list shared items from")
                        .required(true)
                        .index(3),
                ),
        )
        .get_matches();

    let encrypted_file_path = matches
        .value_of("encrypted_file_path")
        .map(String::from)
        .or_else(|| env::var("FILE").ok())
        .expect("encrypted_file_path is required");

    let passphrase = matches
        .value_of("passphrase")
        .map(String::from)
        .or_else(|| env::var("PASSPHRASE").ok());

    let subscription_manager_subs = ShinkaiManagerForSubs::initialize_from_encrypted_file_path(
        Path::new(&encrypted_file_path),
        passphrase.as_deref().unwrap_or(""),
    )
    .expect("Failed to initialize ShinkaiManagerForSync");

    let subscription_manager = SubscriptionManager::new(subscription_manager_subs).await;

    if matches.subcommand_matches("check_node_health").is_some() {
        match subscription_manager.check_node_health().await {
            Ok(status) => println!("Node health status: {:?}", status),
            Err(e) => eprintln!("Error checking node health: {}", e),
        }
    } else if let Some(matches) = matches.subcommand_matches("get_my_node_folder") {
        let path = matches.value_of("path").unwrap();
        match subscription_manager.get_my_node_folder(path.to_string()).await {
            Ok(folder) => println!("Folder details: \n{}", folder),
            Err(e) => eprintln!("Error getting folder: {:?}", e),
        }
    } else if let Some(matches) = matches.subcommand_matches("get_my_node_folder_raw") {
        let path = matches.value_of("path").unwrap();
        match subscription_manager.get_my_node_folder_raw(path.to_string()).await {
            Ok(folder) => println!("Folder details: {}", folder),
            Err(e) => eprintln!("Error getting folder: {:?}", e),
        }
    } else if let Some(matches) = matches.subcommand_matches("create_folder") {
        let folder_name = matches.value_of("folder_name").unwrap();
        let path = matches.value_of("path").unwrap();
        match subscription_manager
            .create_folder(folder_name.to_string(), path.to_string())
            .await
        {
            Ok(_) => println!("Folder created successfully"),
            Err(e) => eprintln!("Error creating folder: {}", e),
        }
    } else if let Some(matches) = matches.subcommand_matches("share_folder") {
        let path = matches.value_of("path").unwrap();
        let minimum_token_delegation = matches
            .value_of("minimum_token_delegation")
            .and_then(|v| v.parse::<u64>().ok());
        let minimum_time_delegated_hours = matches
            .value_of("minimum_time_delegated_hours")
            .and_then(|v| v.parse::<u64>().ok());
        let monthly_payment = matches
            .value_of("monthly_payment")
            .and_then(|v| serde_json::from_str(v).ok()); // Assuming PaymentOption can be parsed from JSON
        let is_free = matches
            .value_of("is_free")
            .expect("is_free is required")
            .parse::<bool>()
            .expect("Invalid value for is_free");

        let has_web_alternative = matches
            .value_of("has_web_alternative")
            .map(|v| v.parse::<bool>().unwrap())
            .or_else(|| {
                env::var("HAS_WEB_ALTERNATIVE")
                    .ok()
                    .and_then(|v| v.parse::<bool>().ok())
            });

        let folder_description = matches
            .value_of("folder_description")
            .map(String::from)
            .or_else(|| env::var("FOLDER_DESCRIPTION").ok())
            .unwrap_or_else(|| "Default folder description".to_string());

        let req = FolderSubscription {
            minimum_token_delegation,
            minimum_time_delegated_hours,
            monthly_payment,
            is_free,
            has_web_alternative,
            folder_description,
        };

        let file_credentials = if let Some(true) = has_web_alternative {
            let source = matches
                .value_of("source")
                .map(String::from)
                .or_else(|| env::var("SOURCE").ok())
                .unwrap_or_else(|| {
                    eprintln!("Source is required when web alternative is enabled");
                    process::exit(1); // Exit the program with an error code
                });

            let access_key_id = matches
                .value_of("access_key_id")
                .map(String::from)
                .or_else(|| env::var("ACCESS_KEY_ID").ok())
                .unwrap_or_else(|| {
                    eprintln!("Access key ID is required when web alternative is enabled");
                    process::exit(1); // Exit the program with an error code
                });

            let secret_access_key = matches
                .value_of("secret_access_key")
                .map(String::from)
                .or_else(|| env::var("SECRET_ACCESS_KEY").ok())
                .unwrap_or_else(|| {
                    eprintln!("Secret access key is required when web alternative is enabled");
                    process::exit(1); // Exit the program with an error code
                });

            let endpoint_uri = matches
                .value_of("endpoint_uri")
                .map(String::from)
                .or_else(|| env::var("ENDPOINT_URI").ok())
                .unwrap_or_else(|| {
                    eprintln!("Endpoint URI is required when web alternative is enabled");
                    process::exit(1); // Exit the program with an error code
                });

            let bucket = matches
                .value_of("bucket")
                .map(String::from)
                .or_else(|| env::var("BUCKET").ok())
                .unwrap_or_else(|| {
                    eprintln!("Bucket is required when web alternative is enabled");
                    process::exit(1); // Exit the program with an error code
                });

            Some(FileDestinationCredentials::new(
                source,
                access_key_id,
                secret_access_key,
                endpoint_uri,
                bucket,
            ))
        } else {
            None
        };

        match file_credentials {
            Some(credentials) => {
                match subscription_manager
                    .share_folder(path.to_string(), req, Some(credentials))
                    .await
                {
                    Ok(_) => println!("Folder shared successfully"),
                    Err(e) => eprintln!("Error sharing folder: {}", e),
                }
            }
            None => {
                if has_web_alternative.unwrap_or(false) {
                    eprintln!("Error: Missing required parameters for web alternative");
                } else {
                    match subscription_manager.share_folder(path.to_string(), req, None).await {
                        Ok(_) => println!("Folder shared successfully"),
                        Err(e) => eprintln!("Error sharing folder: {}", e),
                    }
                }
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("subscribe_to_folder") {
        let path = matches.value_of("path").unwrap();
        let node_name = matches.value_of("node_name").unwrap();
        let profile_name = matches.value_of("profile_name").unwrap();
        let http_preferred = matches.value_of("http_preferred").map(|v| v.parse::<bool>().unwrap());
        let base_folder = matches.value_of("base_folder").map(String::from);
        let subscription_req: SubscriptionPayment = SubscriptionPayment::Free;
        match subscription_manager
            .subscribe_to_folder(
                path.to_string(),
                node_name.to_string(),
                profile_name.to_string(),
                subscription_req,
                http_preferred,
                base_folder,
            )
            .await
        {
            Ok(_) => println!("Subscribed to folder successfully"),
            Err(e) => eprintln!("Error subscribing to folder: {}", e),
        }
    } else if matches.subcommand_matches("my_subscriptions").is_some() {
        match subscription_manager.my_subscriptions().await {
            Ok(resp) => println!("List of subscriptions: {:?}", resp),
            Err(e) => eprintln!("Error listing subscriptions: {}", e),
        }
    } else if matches.subcommand_matches("my_shared_folders").is_some() {
        match subscription_manager.my_shared_folders().await {
            Ok(resp) => println!("List of shared folders: {:?}", resp),
            Err(e) => eprintln!("Error listing shared folders: {}", e),
        }
    } else if let Some(matches) = matches.subcommand_matches("available_shared_items") {
        let path = matches.value_of("path").unwrap();
        let node_name = matches.value_of("node_name").unwrap();
        let profile_name = matches.value_of("profile_name").unwrap();
        match subscription_manager
            .available_shared_items(path.to_string(), node_name.to_string(), profile_name.to_string())
            .await
        {
            Ok(resp) => println!("List of available shared items: {:?}", resp),
            Err(e) => eprintln!("Error listing available shared items: {}", e),
        }
    }
}
