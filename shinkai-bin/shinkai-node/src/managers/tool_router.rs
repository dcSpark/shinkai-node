use super::IdentityManager;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::GenericInferenceChain;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, InferenceChainContextTrait};
use crate::llm_provider::job_manager::JobManager;
use crate::network::node_shareable_logic::ZipFileContents;
use crate::network::zip_export_import::zip_export_import::{
    get_agent_from_zip, get_tool_from_zip, import_agent, import_tool
};
use crate::network::Node;
use crate::tools::tool_definitions::definition_generation::{generate_tool_definitions, get_rust_tools};
use crate::tools::tool_execution::{
    execute_agent_dynamic::execute_agent_tool, execution_coordinator::override_tool_config, execution_custom::try_to_execute_rust_tool, execution_header_generator::{check_tool, generate_execution_environment}
};
use crate::utils::environment::{fetch_node_environment, NodeEnvironment};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::{
    indexable_version::IndexableVersion, invoices::{Invoice, InvoiceStatusEnum}, job::JobLike, llm_providers::common_agent_llm_provider::ProviderOrAgent, shinkai_name::ShinkaiName, shinkai_preferences::ShinkaiInternalComms, shinkai_tool_offering::{AssetPayment, ToolPrice, UsageType, UsageTypeInquiry}, tool_router_key::ToolRouterKey, wallet_mixed::{Asset, NetworkIdentifier}, ws_types::{PaymentMetadata, WSMessageType, WidgetMetadata}
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{AssociatedUI, WSTopic};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::files::prompts_data;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    error::ToolError, network_tool::NetworkTool, parameters::Parameters, rust_tools::RustTool, shinkai_tool::{ShinkaiTool, ShinkaiToolHeader}, tool_config::ToolConfig, tool_output_arg::ToolOutputArg
};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(Clone)]
pub struct ToolRouter {
    pub sqlite_manager: Arc<SqliteManager>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
    pub signing_secret_key: SigningKey,
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    pub default_tool_router_keys: Arc<Mutex<Vec<String>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunctionResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

impl ToolRouter {
    pub fn new(
        sqlite_manager: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        job_manager: Option<Arc<Mutex<JobManager>>>,
    ) -> Self {
        ToolRouter {
            sqlite_manager,
            identity_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
            job_manager,
            default_tool_router_keys: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn initialization(&self, embedding_generator: Arc<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let is_empty;
        let has_any_js_tools;
        {
            is_empty = self
                .sqlite_manager
                .is_empty()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

            has_any_js_tools = self
                .sqlite_manager
                .has_any_js_tools()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        println!(
            "Initializing tool router - Database empty: {}, Has JS tools: {}",
            is_empty, has_any_js_tools
        );

        if let Err(e) = self.add_rust_tools().await {
            eprintln!("Error adding rust tools: {}", e);
        }

        let node_env = fetch_node_environment();
        let node_name: String = node_env.global_identity_name.clone();
        let full_identity =
            ShinkaiName::new(format!("{}/main", node_name)).map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        println!("Importing tools from directory for node: {}", node_name);
        if let Err(e) = Self::import_tools_from_directory(
            self.sqlite_manager.clone(),
            full_identity,
            self.signing_secret_key.clone(),
            self.default_tool_router_keys.clone(),
            embedding_generator.clone(),
        )
        .await
        {
            eprintln!("Error importing tools from directory: {}", e);
        }

        if is_empty {
            println!("Database is empty, adding static prompts and testing network tools");
            if let Err(e) = self.add_static_prompts(embedding_generator).await {
                eprintln!("Error adding static prompts: {}", e);
            }
            if let Err(e) = self.add_testing_network_tools().await {
                eprintln!("Error adding testing network tools: {}", e);
            }
        } else if !has_any_js_tools {
            println!("No JS tools found, adding testing network tools");
            if let Err(e) = self.add_testing_network_tools().await {
                eprintln!("Error adding testing network tools: {}", e);
            }
        }

        Ok(())
    }

    pub async fn force_reinstall_all(&self, embedding_generator: Arc<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        if let Err(e) = self.add_rust_tools().await {
            eprintln!("Error adding rust tools: {}", e);
        }
        if let Err(e) = self.add_static_prompts(embedding_generator.clone()).await {
            eprintln!("Error adding static prompts: {}", e);
        }

        let node_env = fetch_node_environment();
        let full_identity = ShinkaiName::new(format!("{}/main", node_env.global_identity_name))
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        if let Err(e) = Self::import_tools_from_directory(
            self.sqlite_manager.clone(),
            full_identity,
            self.signing_secret_key.clone(),
            self.default_tool_router_keys.clone(),
            embedding_generator,
        )
        .await
        {
            eprintln!("Error importing tools from directory: {}", e);
        }
        if let Err(e) = self.add_testing_network_tools().await {
            eprintln!("Error adding testing network tools: {}", e);
        }
        Ok(())
    }

    pub async fn sync_tools_from_directory(
        &self,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
    ) -> Result<(), ToolError> {
        let node_env = fetch_node_environment();
        let full_identity = ShinkaiName::new(format!("{}/main", node_env.global_identity_name))
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        if let Err(e) = Self::import_tools_from_directory(
            self.sqlite_manager.clone(),
            full_identity,
            self.signing_secret_key.clone(),
            self.default_tool_router_keys.clone(),
            embedding_generator.clone(),
        )
        .await
        {
            eprintln!("Error importing tools from directory: {}", e);
        }
        Ok(())
    }

    async fn import_from_local_directory(
        db: Arc<SqliteManager>,
        full_identity: ShinkaiName,
        node_env: NodeEnvironment,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
    ) -> Result<(), ToolError> {
        let directory_path = match env::var("INSTALL_FOLDER_PATH") {
            Ok(path) => PathBuf::from(&path),
            Err(_) => {
                eprintln!("INSTALL_FOLDER_PATH is not set, skipping import from local directory");
                return Ok(());
            }
        };

        if !directory_path.exists() {
            eprintln!("Install directory not found: {}", directory_path.display());
            return Ok(());
        }

        let files = std::fs::read_dir(directory_path).map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        for file in files {
            let file = file.unwrap();
            let file_path = file.path();
            if !file_path.is_file() {
                println!("Skipping non-file: {}", file_path.display());
                continue;
            }

            let file_extension = file_path.extension().unwrap_or_default();
            if file_extension != "zip" {
                println!("Skipping non-zip file: {}", file_path.display());
                continue;
            }

            let file_content =
                std::fs::read(file_path.clone()).map_err(|e| ToolError::ExecutionError(e.to_string()))?;
            let cursor = std::io::Cursor::new(file_content);
            let mut archive = match zip::ZipArchive::new(cursor) {
                Ok(archive) => archive,
                Err(e) => {
                    eprintln!("Error opening zip file: {}", e);
                    continue;
                }
            };
            let is_agent = match archive.by_name("__agent.json") {
                Ok(_) => true,
                Err(_) => false,
            };

            let is_tool = match archive.by_name("__tool.json") {
                Ok(_) => true,
                Err(_) => false,
            };
            if (is_agent && is_tool) || (!is_agent && !is_tool) {
                eprintln!("Invalid zip file {}", file_path.clone().display());
                continue;
            }

            if is_agent {
                let agent = get_agent_from_zip(archive.clone()).map_err(|e| ToolError::ExecutionError(e.message))?;
                let import_result = import_agent(
                    db.clone(),
                    full_identity.clone(),
                    archive.clone(),
                    agent,
                    embedding_generator.clone(),
                )
                .await;
                if let Err(e) = import_result {
                    eprintln!("Error importing agent: {:?}", e);
                }
            }
            if is_tool {
                let tool = get_tool_from_zip(archive.clone()).map_err(|e| ToolError::ExecutionError(e.message))?;
                let tool_archive = ZipFileContents {
                    buffer: serde_json::to_vec(&tool).unwrap(),
                    archive: archive.clone(),
                };
                let import_result = import_tool(db.clone(), node_env.clone(), tool_archive, tool).await;
                if let Err(e) = import_result {
                    eprintln!("Error importing tool: {:?}", e);
                }
            }
        }

        Ok(())
    }

    /// Attempts to import each tool from a remote directory JSON.
    /// Now also checks if a tool is installed with an older version, and if so, calls `upgrade_tool`.
    async fn import_tools_from_directory(
        db: Arc<SqliteManager>,
        full_identity: ShinkaiName,
        signing_secret_key: SigningKey,
        default_tool_router_keys: Arc<Mutex<Vec<String>>>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
    ) -> Result<(), ToolError> {
        let node_env = fetch_node_environment();

        if env::var("SKIP_IMPORT_FROM_DIRECTORY")
            .unwrap_or("false".to_string())
            .to_lowercase()
            .eq("true")
        {
            println!("Skipping directory imports due to SKIP_IMPORT_FROM_DIRECTORY flag");
            let internal_comms = ShinkaiInternalComms {
                internal_has_sync_default_tools: true,
            };
            if let Err(e) = db.set_preference(
                "internal_comms",
                &internal_comms,
                Some("Internal communication preferences"),
            ) {
                eprintln!("Error setting internal_comms preference: {}", e);
            }
            return Ok(());
        }

        println!("Starting import from local directory");
        Self::import_from_local_directory(
            db.clone(),
            full_identity.clone(),
            node_env.clone(),
            embedding_generator.clone(),
        )
        .await?;

        // Set the sync status to false at the start
        let internal_comms = ShinkaiInternalComms {
            internal_has_sync_default_tools: false,
        };
        if let Err(e) = db.set_preference(
            "internal_comms",
            &internal_comms,
            Some("Internal communication preferences"),
        ) {
            eprintln!("Error setting internal_comms preference: {}", e);
        }

        let start_time = Instant::now();
        let node_env = fetch_node_environment();

        let url = env::var("SHINKAI_TOOLS_DIRECTORY_URL")
            .unwrap_or_else(|_| format!("https://store-api.shinkai.com/store/defaults"));

        println!("Fetching tools from remote directory: {}", url);
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
            .send()
            .await
            .map_err(|e| ToolError::RequestError(e))?;

        if response.status() != 200 {
            return Err(ToolError::ExecutionError(format!(
                "Import tools request returned a non OK status: {}",
                response.status()
            )));
        }

        let tools: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| ToolError::ParseError(format!("Failed to parse tools directory: {}", e)))?;

        println!("Found {} tools in remote directory", tools.len());

        // Collect default tool router keys
        let default_tool_keys: Vec<String> = tools
            .iter()
            .filter_map(|tool| {
                let router_key = tool["routerKey"].as_str()?;
                let is_default = tool["isDefault"].as_bool().unwrap_or(false);
                if is_default {
                    Some(router_key.to_owned())
                } else {
                    None
                }
            })
            .collect();

        println!("Found {} default tool router keys", default_tool_keys.len());

        // Store the default tool keys in the ToolRouter
        {
            let mut default_keys = default_tool_router_keys.lock().await;
            *default_keys = default_tool_keys;
            println!("Stored {} default tool router keys", default_keys.len());
        }

        // Each entry must have "name", "file", and "routerKey" at minimum, plus optional "version".
        // E.g. { "name": "xyz", "file": "...", "routerKey": "...", "version": "2.1.0" }
        let tool_infos = tools
            .iter()
            .filter_map(|tool| {
                let name = tool["name"].as_str()?;
                let file = tool["file"].as_str()?;
                let router_key = tool["routerKey"].as_str()?;
                // It's OK if no version is specified in the JSON; default to 1.0.0
                let version = tool["version"].as_str().unwrap_or("1.0.0").to_owned();
                let r#type = tool["type"].as_str().unwrap_or("").to_owned();
                Some((name.to_owned(), file.to_owned(), router_key.to_owned(), version, r#type))
            })
            .collect::<Vec<_>>();

        println!("Processing {} tools in chunks", tool_infos.len());
        let mut tools_added = 0;
        let mut tools_skipped = 0;
        let mut tools_failed = 0;

        let chunk_size = 5;
        for (chunk_index, chunk) in tool_infos.chunks(chunk_size).enumerate() {
            println!(
                "Processing chunk {}/{}",
                chunk_index + 1,
                (tool_infos.len() + chunk_size - 1) / chunk_size
            );
            let futures = chunk
                .iter()
                .map(|(tool_name, tool_url, router_key, new_version, r#type)| {
                    let db = db.clone();
                    let node_env = node_env.clone();
                    let full_identity = full_identity.clone();
                    let signing_secret_key: SigningKey = signing_secret_key.clone();
                    let embedding_generator = embedding_generator.clone();
                    async move {
                        if r#type == "Tool" {
                            // Try to see if a tool with the same routerKey is already installed.
                            let do_install = match db.get_tool_by_key(router_key) {
                                Ok(existing_tool) => {
                                    // Compare version numbers:
                                    // The local version is existing_tool.version(),
                                    // the remote version is new_version (string from the JSON).
                                    // We parse them into IndexableVersion and compare.
                                    let local_ver = existing_tool.version_indexable()?;
                                    let remote_ver = IndexableVersion::from_string(new_version)?;
                                    Ok(remote_ver > local_ver)
                                }
                                Err(SqliteManagerError::ToolNotFound(_)) => Ok(true),
                                Err(e) => Err(ToolError::DatabaseError(e.to_string())),
                            }?;

                            if !do_install {
                                tools_skipped += 1;
                                return Ok::<(), ToolError>(());
                            }

                            let val: Value = Node::v2_api_import_tool_url_internal(
                                db.clone(),
                                full_identity.clone(),
                                node_env.clone(),
                                tool_url.to_string(),
                                signing_secret_key,
                                embedding_generator,
                            )
                            .await
                            .map_err(|e| ToolError::ExecutionError(e.message))?;

                            // We stored the tool under val["tool"] in the JSON response
                            match serde_json::from_value::<ShinkaiTool>(val["tool"].clone()) {
                                Ok(_tool) => {
                                    tools_added += 1;
                                    println!("Successfully imported tool {} (version: {})", tool_name, new_version);
                                }
                                Err(err) => {
                                    tools_failed += 1;
                                    eprintln!("Couldn't parse 'tool' field as ShinkaiTool: {}", err);
                                }
                            }
                        } else if r#type == "Agent" {
                            let tool_router_key = ToolRouterKey::from_string(router_key)?;
                            let agent_id = tool_router_key.name;
                            let do_install = match db.get_agent(&agent_id) {
                                Ok(agent) => match agent {
                                    Some(_) => Ok(false),
                                    None => Ok(true),
                                },
                                Err(e) => Err(ToolError::DatabaseError(e.to_string())),
                            }?;
                            if !do_install {
                                tools_skipped += 1;
                                return Ok::<(), ToolError>(());
                            }

                            let val: Value = Node::v2_api_import_agent_url_internal(
                                db.clone(),
                                tool_url.to_string(),
                                full_identity.clone(),
                                node_env.clone(),
                                signing_secret_key,
                                embedding_generator,
                            )
                            .await
                            .map_err(|e| ToolError::ExecutionError(e.message))?;

                            match serde_json::from_value::<Agent>(val["agent"].clone()) {
                                Ok(agent) => {
                                    tools_added += 1;
                                    println!("Successfully imported agent {}", agent.name);
                                }
                                Err(err) => {
                                    tools_failed += 1;
                                    eprintln!("Couldn't parse 'agent' field as Agent: {}", err);
                                }
                            }
                        }
                        Ok::<(), ToolError>(())
                    }
                });
            futures::future::join_all(futures).await;
        }

        let duration = start_time.elapsed();
        println!("Tool import summary:");
        println!("- Total tools processed: {}", tool_infos.len());
        println!("- Tools added: {}", tools_added);
        println!("- Tools skipped: {}", tools_skipped);
        println!("- Tools failed: {}", tools_failed);
        println!("- Total time taken: {:?}", duration);

        let internal_comms = ShinkaiInternalComms {
            internal_has_sync_default_tools: true,
        };
        if let Err(e) = db.set_preference(
            "internal_comms",
            &internal_comms,
            Some("Internal communication preferences"),
        ) {
            eprintln!("Error setting internal_comms preference: {}", e);
        }

        Ok(())
    }

    pub async fn add_static_prompts(&self, _: Arc<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Check if ONLY_TESTING_PROMPTS is set
        if env::var("ONLY_TESTING_PROMPTS").unwrap_or_default() == "1"
            || env::var("ONLY_TESTING_PROMPTS").unwrap_or_default().to_lowercase() == "true"
        {
            return Ok(()); // Return right away and don't add anything
        }

        let start_time = Instant::now();

        // Determine which set of prompts to use
        let prompts_data = if env::var("IS_TESTING").unwrap_or_default() == "1" {
            prompts_data::PROMPTS_JSON_TESTING
        } else {
            prompts_data::PROMPTS_JSON
        };

        // Parse the JSON string into a Vec<Value>
        let json_array: Vec<Value> = serde_json::from_str(prompts_data).expect("Failed to parse prompts JSON data");

        println!("Number of static prompts to add: {}", json_array.len());

        // Use the add_prompts_from_json_values method
        {
            self.sqlite_manager
                .add_prompts_from_json_values(json_array)
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        let duration = start_time.elapsed();
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            println!("Time taken to add static prompts: {:?}", duration);
        }
        Ok(())
    }

    pub async fn add_network_tool(&self, network_tool: NetworkTool) -> Result<(), ToolError> {
        self.sqlite_manager
            .add_tool(ShinkaiTool::Network(network_tool, true))
            .await
            .map(|_| ())
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    pub async fn add_rust_tools(&self) -> Result<(), ToolError> {
        let rust_tools = get_rust_tools();
        println!("Adding {} Rust tools", rust_tools.len());
        let mut added_count = 0;
        let mut skipped_count = 0;

        for tool in rust_tools {
            let rust_tool = RustTool::new(
                tool.name,
                tool.description,
                tool.input_args,
                tool.output_arg,
                None,
                tool.tool_router_key,
            );

            let _ = match self.sqlite_manager.get_tool_by_key(&rust_tool.tool_router_key) {
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    added_count += 1;
                    self.sqlite_manager
                        .add_tool(ShinkaiTool::Rust(rust_tool, true))
                        .await
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))
                }
                Err(e) => Err(ToolError::DatabaseError(e.to_string())),
                Ok(_db_tool) => {
                    skipped_count += 1;
                    continue;
                }
            }?;
        }
        println!(
            "Rust tools installation complete - Added: {}, Skipped: {}",
            added_count, skipped_count
        );
        Ok(())
    }

    async fn add_testing_network_tools(&self) -> Result<(), ToolError> {
        // Check if ADD_TESTING_EXTERNAL_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_EXTERNAL_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            let usage_type = UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }]));

            // Manually create NetworkTool
            let network_tool = NetworkTool {
                name: "network__echo".to_string(),
                description: "Echoes the input message".to_string(),
                version: "0.1".to_string(),
                mcp_enabled: Some(false),
                provider: ShinkaiName::new("@@agent_provider.sep-shinkai".to_string()).unwrap(),
                author: "@@official.shinkai".to_string(),
                usage_type: usage_type.clone(),
                activated: true,
                config: vec![],
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property(
                        "message".to_string(),
                        "string".to_string(),
                        "The message to echo".to_string(),
                        true,
                        None,
                    );
                    params
                },
                output_arg: ToolOutputArg { json: "".to_string() },
                embedding: None,
                restrictions: None,
            };
            {
                let shinkai_tool = ShinkaiTool::Network(network_tool, true);

                self.sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }

            // Manually create another NetworkTool
            let youtube_tool = NetworkTool {
                name: "youtube_transcript_with_timestamps".to_string(),
                description: "Takes a YouTube link and summarizes the content by creating multiple sections with a summary and a timestamp.".to_string(),
                version: "0.1".to_string(),
                mcp_enabled: Some(false),
                provider: ShinkaiName::new("@@agent_provider.sep-shinkai".to_string()).unwrap(),
                author: "@@official.shinkai".to_string(),
                usage_type: usage_type.clone(),
                activated: true,
                config: vec![],
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("url".to_string(), "string".to_string(), "The YouTube link to summarize".to_string(), true, None);
                    params
                },
                output_arg: ToolOutputArg { json: "".to_string() },
                embedding: None,
                restrictions: None,
            };

            {
                let shinkai_tool = ShinkaiTool::Network(youtube_tool, true);
                self.sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }
        }

        // Check if ADD_TESTING_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            match self
                .sqlite_manager
                .get_tool_by_key("local:::shinkai-tool-echo:::shinkai__echo")
            {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        js_tool.name = "network__echo".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        self.sqlite_manager
                            .add_tool(modified_tool)
                            .await
                            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                    }
                }
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    eprintln!("Tool not found: local:::shinkai-tool-echo:::shinkai__echo");
                    // Handle the case where the tool is not found, if necessary
                }
                Err(e) => {
                    return Err(ToolError::DatabaseError(e.to_string()));
                }
            }

            match self
                .sqlite_manager
                .get_tool_by_key("local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript")
            {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        js_tool.name = "youtube_transcript_with_timestamps".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        self.sqlite_manager
                            .add_tool(modified_tool)
                            .await
                            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                    }
                }
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    eprintln!("Tool not found: local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript");
                    // Handle the case where the tool is not found, if necessary
                }
                Err(e) => {
                    return Err(ToolError::DatabaseError(e.to_string()));
                }
            }
        }

        Ok(())
    }

    pub async fn get_tool_by_name(&self, name: &str) -> Result<Option<ShinkaiTool>, ToolError> {
        match self.sqlite_manager.get_tool_by_key(name) {
            Ok(tool) => Ok(Some(tool)),
            Err(SqliteManagerError::ToolNotFound(_)) => Ok(None),
            Err(e) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }

    pub async fn get_tool_by_name_and_version(
        &self,
        name: &str,
        version: Option<IndexableVersion>,
    ) -> Result<Option<ShinkaiTool>, ToolError> {
        match self.sqlite_manager.get_tool_by_key_and_version(name, version) {
            Ok(tool) => Ok(Some(tool)),
            Err(SqliteManagerError::ToolNotFound(_)) => Ok(None),
            Err(e) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }

    pub async fn vector_search_enabled_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, false, false)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn vector_search_enabled_tools_with_network(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, false, true)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn vector_search_all_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, true, true)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
        node_name: ShinkaiName,
    ) -> Result<ToolCallFunctionResponse, LLMProviderError> {
        let _function_name = function_call.name.clone();
        let function_args = function_call.arguments.clone();

        // Get additional files
        // Merge agent scope fs_files_paths if llm_provider is an agent
        let mut merged_fs_files_paths = context.fs_files_paths().clone();
        let mut merged_fs_folder_paths = Vec::new();
        if let ProviderOrAgent::Agent(agent) = context.llm_provider() {
            merged_fs_files_paths.extend(agent.scope.vector_fs_items.clone());
            merged_fs_folder_paths.extend(agent.scope.vector_fs_folders.clone());
        }
        let additional_files = GenericInferenceChain::get_additional_files(
            &context.db(),
            &context.full_job(),
            context.job_filenames().clone(),
            merged_fs_files_paths.clone(),
            merged_fs_folder_paths.clone(),
        )?;

        let mut all_files = vec![];
        // Add job scope files
        let job_scope =
            ShinkaiFileManager::get_absolute_path_for_job_scope(&context.db(), &context.full_job().job_id());
        if let Ok(job_scope) = job_scope {
            all_files.extend(job_scope);
        }

        println!("call_function additional_files: {:?}", additional_files);
        println!("call_function job_scope files: {:?}", all_files);
        println!("call_function function_args: {:?}", function_args);

        // Use a HashSet to ensure unique paths
        let mut unique_files: std::collections::HashSet<_> = all_files.into_iter().collect();
        unique_files.extend(additional_files.into_iter());
        let all_files: Vec<_> = unique_files.into_iter().collect();

        let agent_id = if let ProviderOrAgent::Agent(agent) = context.agent() {
            Some(agent.clone().agent_id)
        } else {
            None
        };

        // If agent_id is provided, get the agent's tool config overrides and merge with extra_config
        let function_config = shinkai_tool.get_config_from_env();
        let mut function_config_vec: Vec<ToolConfig> = function_config.into_iter().collect();
        let agent = context.agent().clone();
        match agent {
            ProviderOrAgent::Agent(agent) => {
                if agent_id.is_some() {
                    function_config_vec = override_tool_config(
                        shinkai_tool.tool_router_key().to_string_without_version().clone(),
                        agent,
                        function_config_vec.clone(),
                    );
                }
            }
            _ => {}
        }

        match shinkai_tool {
            ShinkaiTool::Python(python_tool, _is_enabled) => {
                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

                // Get app_id from Cron UI if present, otherwise use job_id
                let app_id = match context.full_job().associated_ui().as_ref() {
                    Some(AssociatedUI::Cron(cron_id)) => cron_id.clone(),
                    _ => context.full_job().job_id().to_string(),
                };

                let tool_id = shinkai_tool.tool_router_key().to_string_without_version().clone();
                let tools: Vec<ToolRouterKey> = context
                    .db()
                    .clone()
                    .get_all_tool_headers()?
                    .into_iter()
                    .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                        Ok(tool_router_key) => Some(tool_router_key),
                        Err(_) => None,
                    })
                    .collect();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Python, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|e| {
                            ToolError::ExecutionError(format!("Failed to generate tool definitions: {:?}", e))
                        })?;

                let envs = generate_execution_environment(
                    context.db(),
                    context.agent().clone().get_id().to_string(),
                    tool_id.clone(),
                    app_id.clone(),
                    agent_id,
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    app_id.clone(),
                    &python_tool.oauth,
                )
                .await?;

                check_tool(
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    python_tool.config.clone(),
                    function_args.clone(),
                    python_tool.input_args.clone(),
                    &python_tool.oauth,
                )?;

                let result = python_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        support_files,
                        function_args,
                        function_config_vec,
                        node_storage_path,
                        app_id.clone(),
                        tool_id.clone(),
                        node_name,
                        false,
                        Some(tool_id),
                        Some(all_files),
                    )
                    .await?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Rust(rust_tool, _is_enabled) => {
                // Get app_id from Cron UI if present, otherwise use job_id
                let app_id = match context.full_job().associated_ui().as_ref() {
                    Some(AssociatedUI::Cron(cron_id)) => cron_id.clone(),
                    _ => context.full_job().job_id().to_string(),
                };

                let tool_id = shinkai_tool.tool_router_key().to_string_without_version().clone();

                let db = context.db();
                let llm_provider = context.agent().get_llm_provider_id().to_string();
                let bearer = db.read_api_v2_key().unwrap_or_default().unwrap_or_default();

                let job_callback_manager = context.job_callback_manager();
                let mut job_manager: Option<Arc<Mutex<JobManager>>> = None;
                if let Some(job_callback_manager) = &job_callback_manager {
                    let job_callback_manager = job_callback_manager.lock().await;
                    job_manager = job_callback_manager.job_manager.clone();
                }

                if job_manager.is_none() {
                    return Err(LLMProviderError::FunctionExecutionError(
                        "Job manager is not available".to_string(),
                    ));
                }

                check_tool(
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    vec![],
                    function_args.clone(),
                    rust_tool.input_args.clone(),
                    &None,
                )?;

                let result = try_to_execute_rust_tool(
                    &shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    function_args,
                    tool_id,
                    app_id,
                    function_config_vec,
                    bearer,
                    db.clone(),
                    llm_provider,
                    node_name,
                    self.identity_manager.clone(),
                    job_manager.unwrap(),
                    self.encryption_secret_key.clone(),
                    self.encryption_public_key.clone(),
                    self.signing_secret_key.clone(),
                )
                .await?;

                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Agent(agent_tool, _is_enabled) => {
                let job_callback_manager = context.job_callback_manager();
                let mut job_manager: Option<Arc<Mutex<JobManager>>> = None;
                if let Some(job_callback_manager) = &job_callback_manager {
                    let job_callback_manager = job_callback_manager.lock().await;
                    job_manager = job_callback_manager.job_manager.clone();
                }

                if job_manager.is_none() {
                    return Err(LLMProviderError::FunctionExecutionError(
                        "Job manager is not available".to_string(),
                    ));
                }

                // Clone function_args and inject the agent_id
                let mut modified_function_args = function_args.clone();
                modified_function_args.insert(
                    "agent_id".to_string(),
                    serde_json::Value::String(agent_tool.agent_id.clone()),
                );

                // Use the dedicated execute_agent_tool function
                let result = execute_agent_tool(
                    context.db().read_api_v2_key().unwrap_or_default().unwrap_or_default(),
                    context.db(),
                    modified_function_args,
                    node_name,
                    self.identity_manager.clone(),
                    job_manager.unwrap(),
                    self.encryption_secret_key.clone(),
                    self.encryption_public_key.clone(),
                    self.signing_secret_key.clone(),
                )
                .await
                .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;

                // Convert the result to a JSON string
                let response = serde_json::to_string(&result).unwrap_or_else(|_| {
                    "{\"message\":\"\", \"session_id\":\"\", \"status\":\"some error\"}".to_string()
                });

                return Ok(ToolCallFunctionResponse {
                    response,
                    function_call,
                });
            }
            ShinkaiTool::Deno(deno_tool, _is_enabled) => {
                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

                // Get app_id from Cron UI if present, otherwise use job_id
                let app_id = match context.full_job().associated_ui().as_ref() {
                    Some(AssociatedUI::Cron(cron_id)) => cron_id.clone(),
                    _ => context.full_job().job_id().to_string(),
                };

                let tool_id = shinkai_tool.tool_router_key().to_string_without_version().clone();
                let tools: Vec<ToolRouterKey> = context
                    .db()
                    .clone()
                    .get_all_tool_headers()?
                    .into_iter()
                    .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                        Ok(tool_router_key) => Some(tool_router_key),
                        Err(_) => None,
                    })
                    .collect();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|e| {
                            ToolError::ExecutionError(format!("Failed to generate tool definitions: {:?}", e))
                        })?;

                let envs = generate_execution_environment(
                    context.db(),
                    context.agent().clone().get_id().to_string(),
                    app_id.clone(),
                    tool_id.clone(),
                    agent_id,
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    app_id.clone(),
                    &deno_tool.oauth,
                )
                .await?;

                check_tool(
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    deno_tool.config.clone(),
                    function_args.clone(),
                    deno_tool.input_args.clone(),
                    &deno_tool.oauth,
                )?;

                let result = deno_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        support_files,
                        function_args,
                        function_config_vec,
                        node_storage_path,
                        app_id,
                        tool_id.clone(),
                        node_name,
                        false,
                        Some(tool_id),
                        Some(all_files),
                    )
                    .await?;

                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Network(network_tool, _is_enabled) => {
                eprintln!("network tool with name {:?}", network_tool.name);

                let agent_payments_manager = context.my_agent_payments_manager();
                let (internal_invoice_request, wallet_balances) = {
                    // Start invoice request
                    let my_agent_payments_manager = match &agent_payments_manager {
                        Some(manager) => manager.lock().await,
                        None => {
                            eprintln!("call_function> Agent payments manager is not available");
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                "Agent payments manager is not available",
                            );
                            return Err(LLMProviderError::FunctionExecutionError(
                                "Agent payments manager is not available".to_string(),
                            ));
                        }
                    };

                    // Get wallet balances
                    let balances = match my_agent_payments_manager.get_balances(node_name.clone()).await {
                        Ok(balances) => balances,
                        Err(e) => {
                            eprintln!("Failed to get balances: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                format!("Failed to get balances: {}", e).as_str(),
                            );
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Failed to get balances: {}",
                                e
                            )));
                        }
                    };

                    // Send a Network Request Invoice
                    let invoice_request = match my_agent_payments_manager
                        .network_request_invoice(network_tool.clone(), UsageTypeInquiry::PerUse)
                        .await
                    {
                        Ok(request) => request,
                        Err(e) => {
                            eprintln!("Failed to request invoice: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                format!("Failed to request invoice: {}", e).as_str(),
                            );
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Failed to request invoice: {}",
                                e
                            )));
                        }
                    };
                    (invoice_request, balances)
                };

                eprintln!(
                    "call_function> internal_invoice_request: {:?}",
                    internal_invoice_request
                );

                // TODO: Send ws_message to the frontend saying requesting invoice to X and more context

                // Convert balances to Value
                let balances_value = match serde_json::to_value(&wallet_balances) {
                    Ok(value) => value,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            format!("Failed to convert balances to Value: {}", e).as_str(),
                        );
                        return Err(LLMProviderError::FunctionExecutionError(format!(
                            "Failed to convert balances to Value: {}",
                            e
                        )));
                    }
                };

                // Note: there must be a better way to do this
                // Loop to check for the invoice unique_id
                let start_time = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(300); // 5 minutes
                let interval = std::time::Duration::from_millis(100); // 100ms
                let notification_content: Invoice;

                loop {
                    if start_time.elapsed() > timeout {
                        return Err(LLMProviderError::FunctionExecutionError(
                            "Timeout while waiting for invoice unique_id".to_string(),
                        ));
                    }

                    // Check if the invoice is paid
                    match context.db().get_invoice(&internal_invoice_request.unique_id.clone()) {
                        Ok(invoice) => {
                            eprintln!("invoice found: {:?}", invoice);

                            if invoice.status == InvoiceStatusEnum::Pending {
                                // Process the notification
                                notification_content = invoice;
                                break;
                            }
                        }
                        Err(_e) => {
                            // If invoice is not found, check for InvoiceNetworkError
                            match context
                                .db()
                                .get_invoice_network_error(&internal_invoice_request.unique_id.clone())
                            {
                                Ok(network_error) => {
                                    eprintln!("InvoiceNetworkError found: {:?}", network_error);
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("InvoiceNetworkError details: {:?}", network_error),
                                    );
                                    // Return the user_error_message if available, otherwise a default message
                                    let error_message = network_error
                                        .user_error_message
                                        .unwrap_or_else(|| "Invoice network error encountered".to_string());
                                    return Err(LLMProviderError::FunctionExecutionError(error_message));
                                }
                                Err(_) => {
                                    // Continue waiting if neither invoice nor network error is found
                                }
                            }
                        }
                    }
                    tokio::time::sleep(interval).await;
                }

                // Convert notification_content to Value
                let notification_content_value = match serde_json::to_value(&notification_content) {
                    Ok(value) => value,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            format!("Failed to convert notification_content to Value: {}", e).as_str(),
                        );
                        return Err(LLMProviderError::FunctionExecutionError(format!(
                            "Failed to convert notification_content to Value: {}",
                            e
                        )));
                    }
                };

                // Get the ws from the context
                {
                    let ws_manager = context.ws_manager_trait();

                    if let Some(ws_manager) = &ws_manager {
                        let ws_manager = ws_manager.lock().await;
                        let job = context.full_job();

                        let topic = WSTopic::Widget;
                        let subtopic = job.conversation_inbox_name.to_string();
                        let update = "".to_string();
                        let payment_metadata = PaymentMetadata {
                            tool_key: network_tool.name.clone(),
                            description: network_tool.description.clone(),
                            usage_type: network_tool.usage_type.clone(),
                            invoice_id: internal_invoice_request.unique_id.clone(),
                            invoice: notification_content_value.clone(),
                            function_args: function_args.clone(),
                            wallet_balances: balances_value.clone(),
                            error_message: None,
                        };

                        let widget = WSMessageType::Widget(WidgetMetadata::PaymentRequest(payment_metadata));
                        ws_manager.queue_message(topic, subtopic, update, widget, false).await;
                    } else {
                        return Err(LLMProviderError::FunctionExecutionError(
                            "WS manager is not available".to_string(),
                        ));
                    }
                }

                // Wait for the invoice to be paid for up to 5 minutes
                let start_time = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(300); // 5 minutes
                let interval = std::time::Duration::from_millis(100); // 100ms
                let invoice_result: Invoice;

                loop {
                    if start_time.elapsed() > timeout {
                        // Send a timeout notification via WebSocket
                        {
                            let ws_manager = context.ws_manager_trait();

                            if let Some(ws_manager) = &ws_manager {
                                let ws_manager = ws_manager.lock().await;
                                let job = context.full_job();

                                let topic = WSTopic::Widget;
                                let subtopic = job.conversation_inbox_name.to_string();
                                let update = "Timeout while waiting for invoice payment".to_string();
                                let payment_metadata = PaymentMetadata {
                                    tool_key: network_tool.name.clone(),
                                    description: network_tool.description.clone(),
                                    usage_type: network_tool.usage_type.clone(),
                                    invoice_id: internal_invoice_request.unique_id.clone(),
                                    invoice: notification_content_value.clone(),
                                    function_args: function_args.clone(),
                                    wallet_balances: balances_value.clone(),
                                    error_message: Some(update.clone()),
                                };

                                let widget = WSMessageType::Widget(WidgetMetadata::PaymentRequest(payment_metadata));
                                ws_manager.queue_message(topic, subtopic, update, widget, false).await;
                            }
                        }

                        return Err(LLMProviderError::FunctionExecutionError(
                            "Timeout while waiting for invoice payment".to_string(),
                        ));
                    }

                    // Check if the invoice is paid
                    match context.db().get_invoice(&internal_invoice_request.unique_id.clone()) {
                        Ok(invoice) => {
                            if invoice.status == InvoiceStatusEnum::Processed {
                                invoice_result = invoice;
                                break;
                            }
                        }
                        Err(e) => {
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Error while checking for invoice payment: {}",
                                e
                            )));
                        }
                    }

                    // Sleep for the interval before checking again
                    tokio::time::sleep(interval).await;
                }

                eprintln!("invoice_result: {:?}", invoice_result);

                // Try to parse the result_str and extract the "data" field
                let response = match serde_json::from_str::<serde_json::Value>(
                    &invoice_result.result_str.clone().unwrap_or_default(),
                ) {
                    Ok(parsed) => {
                        if let Some(data) = parsed.get("data") {
                            data.to_string()
                        } else {
                            invoice_result.result_str.clone().unwrap_or_default()
                        }
                    }
                    Err(_) => invoice_result.result_str.clone().unwrap_or_default(),
                };

                eprintln!("parsed response: {:?}", response);

                return Ok(ToolCallFunctionResponse {
                    response,
                    function_call,
                });
            }
        }
    }

    /// This function is used to call a JS function directly
    /// It's very handy for agent-to-agent communication
    pub async fn call_js_function(
        &self,
        function_args: serde_json::Map<String, Value>,
        requester_node_name: ShinkaiName,
        js_tool_name: &str,
    ) -> Result<String, LLMProviderError> {
        let shinkai_tool = self.get_tool_by_name(js_tool_name).await?;

        if shinkai_tool.is_none() {
            return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string()));
        }

        let shinkai_tool = shinkai_tool.unwrap();
        let function_config = shinkai_tool.get_config_from_env();
        let function_config_vec: Vec<ToolConfig> = function_config.into_iter().collect();

        let js_tool = match shinkai_tool.clone() {
            ShinkaiTool::Deno(js_tool, _) => js_tool,
            _ => return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string())),
        };

        let node_env = fetch_node_environment();
        let node_storage_path = node_env
            .node_storage_path
            .clone()
            .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
        let tools: Vec<ToolRouterKey> = self
            .sqlite_manager
            .clone()
            .get_all_tool_headers()?
            .into_iter()
            .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                Ok(tool_router_key) => Some(tool_router_key),
                Err(_) => None,
            })
            .collect();
        let app_id = format!("external_{}", uuid::Uuid::new_v4());
        let tool_id = shinkai_tool.tool_router_key().clone().to_string_without_version();
        let support_files =
            generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                .await
                .map_err(|e| ToolError::ExecutionError(format!("Failed to generate tool definitions: {:?}", e)))?;

        let oauth = match shinkai_tool.clone() {
            ShinkaiTool::Deno(deno_tool, _) => deno_tool.oauth.clone(),
            ShinkaiTool::Python(python_tool, _) => python_tool.oauth.clone(),
            _ => return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string())),
        };

        let env = generate_execution_environment(
            self.sqlite_manager.clone(),
            "".to_string(),
            format!("xid-{}", app_id),
            format!("xid-{}", tool_id),
            None,
            shinkai_tool.tool_router_key().clone().to_string_without_version(),
            // TODO: Pass data from the API
            "".to_string(),
            &oauth,
        )
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        check_tool(
            shinkai_tool.tool_router_key().clone().to_string_without_version(),
            function_config_vec.clone(),
            function_args.clone(),
            shinkai_tool.input_args(),
            &oauth,
        )?;

        let result = js_tool
            .run(
                env,
                node_env.api_listen_address.ip().to_string(),
                node_env.api_listen_address.port(),
                support_files,
                function_args,
                function_config_vec,
                node_storage_path,
                app_id,
                tool_id.clone(),
                // TODO Is this correct?
                requester_node_name,
                true,
                Some(tool_id),
                None,
            )
            .await?;
        let result_str =
            serde_json::to_string(&result).map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;

        return Ok(result_str);
    }

    pub async fn combined_tool_search(
        &self,
        query: &str,
        num_of_results: u64,
        include_disabled: bool,
        include_network: bool,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        // Sanitize the query to handle special characters
        let sanitized_query = query.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");

        // Start the timer for vector search
        let vector_start_time = Instant::now();
        let vector_search_result = self
            .sqlite_manager
            .tool_vector_search(&sanitized_query, num_of_results, include_disabled, include_network)
            .await;
        let vector_elapsed_time = vector_start_time.elapsed();
        println!("Time taken for vector search: {:?}", vector_elapsed_time);

        // Start the timer for FTS search
        let fts_start_time = Instant::now();
        let fts_search_result = self.sqlite_manager.search_tools_fts(&sanitized_query);
        let fts_elapsed_time = fts_start_time.elapsed();
        println!("Time taken for FTS search: {:?}", fts_elapsed_time);

        match (vector_search_result, fts_search_result) {
            (Ok(vector_tools), Ok(fts_tools)) => {
                let mut combined_tools = Vec::new();
                let mut seen_ids = std::collections::HashSet::new();

                // Always add the first FTS result if available
                if let Some(first_fts_tool) = fts_tools.first() {
                    if seen_ids.insert(first_fts_tool.tool_router_key.clone()) {
                        combined_tools.push(first_fts_tool.clone());
                    }
                }

                // Check if the top vector search result has a score under 0.2
                if let Some((tool, _score)) = vector_tools.first() {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Add remaining FTS results
                for tool in fts_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Add remaining vector search results
                for (tool, _) in vector_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Log the result count if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    println!("Number of combined tool results: {}", combined_tools.len());
                }

                Ok(combined_tools)
            }
            (Err(e), _) | (_, Err(e)) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }

    pub async fn get_default_tool_router_keys_as_set(&self) -> std::collections::HashSet<String> {
        let default_keys = self.default_tool_router_keys.lock().await;
        default_keys.iter().cloned().collect()
    }
}
