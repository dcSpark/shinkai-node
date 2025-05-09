use crate::network::node_error::NodeError;
use crate::network::node_shareable_logic::ZipFileContents;
use crate::network::Node;
use crate::utils::environment::NodeEnvironment;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::{FileProcessingMode, ShinkaiFileManager};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::agent_tool_wrapper::AgentToolWrapper;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use zip::ZipArchive;
use zip::{write::FileOptions, ZipWriter};

async fn calculate_zip_dependencies(
    db: Arc<SqliteManager>,
    shinkai_name: ShinkaiName,
    tool_entry_point: Option<ShinkaiTool>,
    agent_entry_point: Option<Agent>,
    agent_dependencies: &mut HashMap<String, Agent>,
    tool_dependencies: &mut HashMap<String, ShinkaiTool>,
) -> Result<(), APIError> {
    if let Some(tool) = tool_entry_point {
        let tool_router_key = tool.tool_router_key().to_string_with_version();

        if tool_dependencies.contains_key(&tool_router_key) {
            // Done, this path has been handled
            return Ok(());
        }
        tool_dependencies.insert(tool_router_key, tool.clone());

        match tool.clone() {
            ShinkaiTool::Deno(_, _) => (),
            ShinkaiTool::Python(_, _) => (),
            ShinkaiTool::Rust(_, _) => (),
            ShinkaiTool::Agent(agent_tool, _) => {
                let agent = match db.get_agent(&agent_tool.agent_id) {
                    Ok(agent) => agent,
                    Err(err) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to get agent dependency: {}", err),
                        });
                    }
                };
                // Recursively add the agent to the dependencies
                Box::pin(calculate_zip_dependencies(
                    db.clone(),
                    shinkai_name.clone(),
                    None,
                    agent,
                    agent_dependencies,
                    tool_dependencies,
                ))
                .await?;
                return Ok(());
            }
            ShinkaiTool::Network(_, _) => (),
        }

        // This tool might have dependendies, so let's check them.
        // Only Deno & Python tools have get_tools()
        for dependency in tool.get_tools() {
            let tool_dependency =
                match db.get_tool_by_key_and_version(&dependency.to_string_without_version(), dependency.version()) {
                    Ok(tool) => tool,
                    Err(err) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to get tool dependency: {}", err),
                        });
                    }
                };
            Box::pin(calculate_zip_dependencies(
                db.clone(),
                shinkai_name.clone(),
                Some(tool_dependency),
                None,
                agent_dependencies,
                tool_dependencies,
            ))
            .await?;
        }
    }

    if let Some(agent) = agent_entry_point {
        let agent_id = agent.agent_id.clone();
        if agent_dependencies.contains_key(&agent_id) {
            // Done, this path has been handled
            return Ok(());
        }
        agent_dependencies.insert(agent_id, agent.clone());

        let agent_tool_wrapper = AgentToolWrapper::new(
            agent.agent_id.clone(),
            agent.name.clone(),
            agent.ui_description.clone(),
            shinkai_name.get_node_name_string(),
            None,
        );

        let shinkai_tool = ShinkaiTool::Agent(agent_tool_wrapper.clone(), true);
        tool_dependencies.insert(
            ShinkaiTool::Agent(agent_tool_wrapper, true)
                .tool_router_key()
                .to_string_with_version(),
            shinkai_tool,
        );

        for tool in agent.tools {
            let tool_dependency =
                match db.get_tool_by_key_and_version(&tool.to_string_without_version(), tool.version()) {
                    Ok(tool) => tool,
                    Err(err) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to get tool dependency: {}", err),
                        });
                    }
                };
            Box::pin(calculate_zip_dependencies(
                db.clone(),
                shinkai_name.clone(),
                Some(tool_dependency),
                None,
                agent_dependencies,
                tool_dependencies,
            ))
            .await?;
        }
    }

    return Ok(());
}

async fn get_dependencies_for_zip(
    db: Arc<SqliteManager>,
    shinkai_name: ShinkaiName,
    node_env: NodeEnvironment,
    agent_dependencies: &HashMap<String, Agent>,
    tool_dependencies: &HashMap<String, ShinkaiTool>,
) -> Result<HashMap<String, Vec<u8>>, NodeError> {
    let mut zip_files = HashMap::new();
    for (agent_id, _) in agent_dependencies {
        let agent_bytes = match Box::pin(generate_agent_zip(
            db.clone(),
            shinkai_name.clone(),
            node_env.clone(),
            agent_id.clone(),
            false,
        ))
        .await
        {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(NodeError {
                    message: format!("Failed to generate agent zip: {}", err.message),
                });
            }
        };

        zip_files.insert(format!("__agents/{}.zip", agent_id.replace(':', "_")), agent_bytes);
    }

    for (tool_key, tool) in tool_dependencies {
        match tool {
            ShinkaiTool::Deno(_, _) => (),
            ShinkaiTool::Python(_, _) => (),
            ShinkaiTool::Rust(_, _) => {
                println!("Not including rust tool in zip");
                continue;
            }
            ShinkaiTool::Agent(_, _) => {
                println!("Not including agent tool in zip");
                continue;
            }
            ShinkaiTool::Network(_, _) => (),
        }

        let tool_bytes = match Box::pin(generate_tool_zip(
            db.clone(),
            shinkai_name.clone(),
            node_env.clone(),
            tool.clone(),
            false,
        ))
        .await
        {
            Ok(bytes) => bytes,
            Err(err) => return Err(NodeError::from(err)),
        };

        zip_files.insert(format!("__tools/{}.zip", tool_key.replace(':', "_")), tool_bytes);
    }

    Ok(zip_files)
}

pub async fn generate_agent_zip(
    db: Arc<SqliteManager>,
    shinkai_name: ShinkaiName,
    node_env: NodeEnvironment,
    agent_id: String,
    add_dependencies: bool,
) -> Result<Vec<u8>, APIError> {
    fn internal_error(err: String) -> APIError {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to generate agent zip: {}", err),
        }
    }

    // Retrieve the agent from the database
    let agent = match db.get_agent(&agent_id) {
        Ok(Some(agent)) => agent,
        Ok(None) => return Err(internal_error(format!("Agent not found: {}", agent_id))),
        Err(err) => return Err(internal_error(format!("Failed to retrieve agent: {}", err))),
    };

    // Serialize the agent to JSON bytes
    let agent_bytes = match serde_json::to_vec(&agent) {
        Ok(bytes) => bytes,
        Err(err) => return Err(internal_error(format!("Failed to serialize agent: {}", err))),
    };

    // Create a temporary zip file
    let name = format!("{}.zip", agent.agent_id.replace(':', "_"));
    let path = std::env::temp_dir().join(&name);
    let file = match File::create(&path) {
        Ok(file) => file,
        Err(err) => return Err(internal_error(format!("Failed to create zip file: {}", err))),
    };

    let zip_files: Result<HashMap<String, Vec<u8>>, APIError> = if add_dependencies {
        // Add the dependencies to the zip file
        let mut tool_dependencies = HashMap::new();
        let mut agent_dependencies = HashMap::new();
        Box::pin(calculate_zip_dependencies(
            db.clone(),
            shinkai_name.clone(),
            None,
            Some(agent.clone()),
            &mut agent_dependencies,
            &mut tool_dependencies,
        ))
        .await?;

        // Remove self from dependencies
        agent_dependencies.remove(&agent_id);
        let agent_tool_wrapper = ShinkaiTool::Agent(
            AgentToolWrapper::new(
                agent.agent_id.clone(),
                agent.name.clone(),
                agent.ui_description.clone(),
                shinkai_name.get_node_name_string(),
                None,
            ),
            true,
        )
        .tool_router_key()
        .to_string_with_version();
        tool_dependencies.remove(&agent_tool_wrapper);
        println!("For agent: {}", agent_id);
        println!("Agent dependencies: {:?}", agent_dependencies);
        println!("Tool dependencies: {:?}", tool_dependencies);

        let mut zip_files = get_dependencies_for_zip(
            db.clone(),
            shinkai_name,
            node_env,
            &agent_dependencies,
            &tool_dependencies,
        )
        .await
        .map_err(|e| internal_error(format!("Failed to add dependencies to zip: {}", e)))?;

        zip_files.insert("__agent.json".to_string(), agent_bytes);
        Ok(zip_files)
    } else {
        let mut zip_files = HashMap::new();
        zip_files.insert("__agent.json".to_string(), agent_bytes);
        Ok(zip_files)
    };
    if let Err(err) = zip_files {
        return Err(err);
    }
    let mut zip_files = zip_files.unwrap();

    for item in agent.scope.vector_fs_items {
        let p = item.full_path();
        if item.exists() {
            let file_bytes = fs::read(p).await.map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read file: {}", e),
            })?;

            zip_files.insert(format!("__knowledge/{}", item.relative_path().to_string()), file_bytes);
        }
    }

    for item in agent.scope.vector_fs_folders {
        if item.clone().exists() {
            let files = std::fs::read_dir(item.clone().path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read file: {}", e),
            })?;

            for entry in files {
                let file = entry.map_err(|e| APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to read file: {}", e),
                })?;
                if file
                    .file_type()
                    .map_err(|e| APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to read file: {}", e),
                    })?
                    .is_file()
                {
                    let file_bytes = fs::read(file.path()).await.map_err(|e| APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to read file: {}", e),
                    })?;

                    zip_files.insert(format!("__knowledge/{}", item.relative_path().to_string()), file_bytes);
                }
            }
        }
    }

    let mut zip = ZipWriter::new(file);
    for (file_name, file_bytes) in zip_files {
        if let Err(err) = zip.start_file::<_, ()>(file_name, FileOptions::default()) {
            return Err(internal_error(format!("Failed to create file in zip: {}", err)));
        }
        if let Err(err) = zip.write_all(&file_bytes) {
            return Err(internal_error(format!("Failed to write file data to zip: {}", err)));
        }
    }
    // Finalize the zip file
    if let Err(err) = zip.finish() {
        return Err(internal_error(format!("Failed to finalize zip file: {}", err)));
    }
    // Read the zip file into memory
    let file_bytes: Vec<u8> = fs::read(&path).await.map_err(|e| APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        error: "Internal Server Error".to_string(),
        message: format!("Failed to read zip file: {}", e),
    })?;

    // Clean up the temporary file
    fs::remove_file(&path).await.map_err(|e| APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        error: "Internal Server Error".to_string(),
        message: format!("Failed to remove temporary file: {}", e),
    })?;

    Ok(file_bytes)
}

pub async fn generate_tool_zip(
    db: Arc<SqliteManager>,
    shinkai_name: ShinkaiName,
    node_env: NodeEnvironment,
    tool: ShinkaiTool,
    add_dependencies: bool,
) -> Result<Vec<u8>, NodeError> {
    let mut tool = tool;
    tool.sanitize_config();

    let tool_bytes = serde_json::to_vec(&tool).unwrap();

    let name = format!(
        "{}.zip",
        tool.tool_router_key().to_string_without_version().replace(':', "_")
    );
    let path = std::env::temp_dir().join(&name);
    let file = File::create(&path).map_err(|e| NodeError::from(e.to_string()))?;

    let zip_files: Result<HashMap<String, Vec<u8>>, NodeError> = if add_dependencies {
        // Add the dependencies to the zip file
        let mut tool_dependencies = HashMap::new();
        let mut agent_dependencies = HashMap::new();
        calculate_zip_dependencies(
            db.clone(),
            shinkai_name.clone(),
            Some(tool.clone()),
            None,
            &mut agent_dependencies,
            &mut tool_dependencies,
        )
        .await
        .map_err(|e| NodeError {
            message: format!("Failed to calculate dependencies: {}", e.message),
        })?;
        // Remove self from dependencies
        tool_dependencies.remove(&tool.tool_router_key().to_string_with_version());
        println!("For tool: {}", tool.tool_router_key().to_string_without_version());
        println!("Agent dependencies: {:?}", agent_dependencies);
        println!("Tool dependencies: {:?}", tool_dependencies);

        let mut zip_files = get_dependencies_for_zip(
            db.clone(),
            shinkai_name,
            node_env.clone(),
            &agent_dependencies,
            &tool_dependencies,
        )
        .await
        .map_err(|e| NodeError {
            message: format!("Failed to add dependencies to zip: {}", e.message),
        })?;
        zip_files.insert("__tool.json".to_string(), tool_bytes);
        Ok(zip_files)
    } else {
        let mut zip_files: HashMap<String, Vec<u8>> = HashMap::new();
        zip_files.insert("__tool.json".to_string(), tool_bytes);
        Ok(zip_files)
    };
    if let Err(err) = zip_files {
        return Err(err);
    }
    let mut zip_files = zip_files.unwrap();

    let assets = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
        .join(".tools_storage")
        .join("tools")
        .join(tool.tool_router_key().convert_to_path());

    if assets.exists() {
        for entry in std::fs::read_dir(assets).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                zip_files.insert(
                    path.file_name().unwrap().to_str().unwrap().to_string(),
                    fs::read(path).await.unwrap(),
                );
            }
        }
    }

    let mut zip = ZipWriter::new(file);
    for (file_name, file_bytes) in zip_files {
        zip.start_file::<_, ()>(file_name, FileOptions::default())
            .map_err(|e| NodeError::from(e.to_string()))?;
        zip.write_all(&file_bytes).map_err(|e| NodeError::from(e.to_string()))?;
    }
    zip.finish().map_err(|e| NodeError::from(e.to_string()))?;

    println!("Zip file created successfully!");
    let file_bytes = fs::read(&path).await?;
    // Delete the zip file after reading it
    fs::remove_file(&path).await?;
    Ok(file_bytes)
}

async fn import_tool_assets(
    tool: ShinkaiTool,
    node_env: NodeEnvironment,
    mut zip_contents: ZipFileContents,
) -> Result<(), APIError> {
    let archive_clone = zip_contents.archive.clone();
    let files = archive_clone.file_names();

    for file in files {
        if file.contains("__MACOSX/") {
            continue;
        }
        if file == "__tool.json" {
            continue;
        }
        if file.starts_with("__agents/") || file.starts_with("__tools/") || file.starts_with("__knowledge/") {
            continue;
        }
        println!("[IMPORTING ASSETS]: {}", file);
        let mut buffer = Vec::new();
        {
            let file = zip_contents.archive.by_name(file);
            let mut tool_file = match file {
                Ok(file) => file,
                Err(_) => {
                    return Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Invalid Tool Archive".to_string(),
                        message: "Archive does not contain tool.json".to_string(),
                    });
                }
            };

            // Read the tool file contents into a buffer
            if let Err(err) = tool_file.read_to_end(&mut buffer) {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Read Error".to_string(),
                    message: format!("Failed to read tool.json contents: {}", err),
                });
            }
        } // `tool_file` goes out of scope here

        let mut file_path = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
            .join(".tools_storage")
            .join("tools")
            .join(tool.tool_router_key().convert_to_path());
        if !file_path.exists() {
            let s = std::fs::create_dir_all(&file_path);
            if s.is_err() {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Failed to create directory".to_string(),
                    message: format!("Failed to create directory: {}", s.err().unwrap()),
                });
            }
        }
        file_path.push(file);
        let s: Result<(), std::io::Error> = std::fs::write(&file_path, &buffer);
        if s.is_err() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Failed to write file".to_string(),
                message: format!("Failed to write file: {}", s.err().unwrap()),
            });
        }
    }
    Ok(())
}

async fn import_agent_knowledge(
    mut zip_contents: ZipArchive<std::io::Cursor<Vec<u8>>>,
    db: Arc<SqliteManager>,
    embedding_generator: Arc<dyn EmbeddingGenerator>,
) -> Result<(), APIError> {
    let archive_clone = zip_contents.clone();
    let files = archive_clone.file_names();
    for file in files {
        if file.starts_with("__knowledge/") {
            let mut buffer = Vec::new();
            {
                println!("[IMPORTING KNOWLEDGE]: {}", file);
                let file: Result<zip::read::ZipFile<'_>, zip::result::ZipError> = zip_contents.by_name(file);
                let mut tool_file = match file {
                    Ok(file) => file,
                    Err(_) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid Tool Archive".to_string(),
                            message: "Archive does not contain tool.json".to_string(),
                        });
                    }
                };

                // Read the tool file contents into a buffer
                if let Err(err) = tool_file.read_to_end(&mut buffer) {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Read Error".to_string(),
                        message: format!("Failed to read tool.json contents: {}", err),
                    });
                }
            } // `tool_file` goes out of scope here

            let relative_path = file.replace("__knowledge/", "");
            let dest_path = ShinkaiPath::from_str(&relative_path);
            ShinkaiFileManager::save_and_process_file(
                dest_path,
                buffer,
                &db,
                FileProcessingMode::Auto,
                &*embedding_generator,
            )
            .await
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Failed to save and process file".to_string(),
                message: format!("Failed to save and process file: {}", e),
            })?;
        }
    }

    Ok(())
}

pub async fn import_dependencies_tools(
    db: Arc<SqliteManager>,
    node_env: NodeEnvironment,
    zip_contents: ZipArchive<std::io::Cursor<Vec<u8>>>,
    embedding_generator: Arc<dyn EmbeddingGenerator>,
) -> Result<(), APIError> {
    // Import tools from the zip file
    let archive_clone = zip_contents.clone();
    let files = archive_clone.file_names();
    for file in files {
        if file.starts_with("__tools/") {
            let tool_zip = match bytes_to_zip_tool(zip_contents.clone(), file.to_string(), true).await {
                Ok(tool_zip) => tool_zip,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Invalid Tool Archive".to_string(),
                        message: format!("Failed to extract tool.json: {:?}", err),
                    };
                    return Err(api_error);
                }
            };

            let tool: ShinkaiTool = match serde_json::from_slice(&tool_zip.buffer) {
                Ok(tool) => tool,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Invalid Tool JSON".to_string(),
                        message: format!("Failed to parse tool.json: {}", err),
                    };
                    return Err(api_error);
                }
            };
            let import_tool_result = import_tool(db.clone(), node_env.clone(), tool_zip, tool).await;
            if let Err(err) = import_tool_result {
                println!("Error importing tool: {:?}", err);
            }
        }
        if file.starts_with("__agents/") {
            let agent_zip = match bytes_to_zip_tool(zip_contents.clone(), file.to_string(), false).await {
                Ok(agent_zip) => agent_zip,
                Err(err) => return Err(err),
            };
            let agent = get_agent_from_zip(agent_zip.archive).unwrap();
            let import_agent_result =
                import_agent(db.clone(), zip_contents.clone(), agent, embedding_generator.clone()).await;
            if let Err(err) = import_agent_result {
                println!("Error importing agent: {:?}", err);
            }
        }
    }
    Ok(())
}

pub async fn import_tool(
    db: Arc<SqliteManager>,
    node_env: NodeEnvironment,
    zip_contents: ZipFileContents,
    tool: ShinkaiTool,
) -> Result<Value, APIError> {
    println!(
        "[IMPORTING TOOL]: {}",
        tool.tool_router_key().to_string_without_version()
    );

    // Check if the tool can be enabled and enable it if possible
    let mut tool = tool.clone();
    if !tool.is_enabled() && tool.can_be_enabled() {
        tool.enable();
    }

    let tool_router_key = tool.tool_router_key().to_string_without_version();
    match tool.clone() {
        ShinkaiTool::Deno(_, _) => {}
        ShinkaiTool::Python(_, _) => {}
        ShinkaiTool::Network(_, _) => {}
        ShinkaiTool::Rust(_, _) => {
            println!("Rust tool detected {}. Skipping installation.", tool_router_key);
            return Ok(json!({
                "status": "success",
                "message": "Tool imported successfully",
                "tool_key": tool_router_key,
                "tool": tool.clone()
            }));
        }
        ShinkaiTool::Agent(_, _) => {
            // TODO Agents might depend on other agents, so we need to handle that.
            println!("Agent tool detected {}. Skipping installation.", tool_router_key);
            return Ok(json!({
                "status": "success",
                "message": "Tool imported successfully",
                "tool_key": tool_router_key,
                "tool": tool.clone()
            }));
        }
    }

    // check if any version of the tool exists in the database
    let db_tool = match db.get_tool_by_key(&tool.tool_router_key().to_string_without_version()) {
        Ok(tool) => Some(tool),
        Err(_) => None,
    };

    // if the tool exists in the database, check if the version is the same or newer
    if let Some(db_tool) = db_tool.clone() {
        let version_db = db_tool.version_number()?;
        let version_zip = tool.version_number()?;
        if version_db >= version_zip {
            // No need to update
            return Ok(json!({
                "status": "success",
                "message": "Tool already up-to-date",
                "tool_key": tool.tool_router_key().to_string_without_version(),
                "tool": tool.clone()
            }));
        }
    }

    // Save the tool to the database
    let tool = match db_tool {
        None => db.add_tool(tool).await.map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Database Error".to_string(),
            message: format!("Failed to save tool to database: {}", e),
        })?,
        Some(_) => db.upgrade_tool(tool).await.map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Database Error".to_string(),
            message: format!("Failed to upgrade tool: {}", e),
        })?,
    };

    import_tool_assets(tool.clone(), node_env.clone(), zip_contents).await?;
    Ok(json!({
        "status": "success",
        "message": "Tool imported successfully",
        "tool_key": tool.tool_router_key().to_string_without_version(),
        "tool": tool
    }))
}

pub async fn import_agent(
    db: Arc<SqliteManager>,
    zip_contents: ZipArchive<std::io::Cursor<Vec<u8>>>,
    mut agent: Agent,
    embedding_generator: Arc<dyn EmbeddingGenerator>,
) -> Result<Value, APIError> {
    println!("[IMPORTING AGENT]: {}", agent.agent_id);
    // Do not overwrite existing agent
    // There is no clear mechanism to determine the latest version of the agent
    // So we just check if the agent exists in the database
    let install = match db.get_agent(&agent.agent_id) {
        Ok(agent) => match agent {
            Some(_) => false,
            None => true,
        },
        Err(_) => true,
    };

    let preferences_llm_provider_result = match db.get_preference::<String>("default_llm_provider") {
        Ok(llm_provider) => match llm_provider {
            Some(llm_provider) => llm_provider,
            None => Node::shinkai_free_provider_id(),
        },
        Err(_) => {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Database Error".to_string(),
                message: "Failed to get default LLM provider".to_string(),
            })
        }
    };

    agent.llm_provider_id = preferences_llm_provider_result;

    if install {
        match db.add_agent(agent.clone(), &agent.full_identity_name) {
            Ok(_) => {
                let author = agent.full_identity_name.node_name.clone();
                let agent_tool_wrapper = AgentToolWrapper::new(
                    agent.agent_id.clone(),
                    agent.name.clone(),
                    agent.ui_description.clone(),
                    author,
                    None,
                );
                let shinkai_tool = ShinkaiTool::Agent(agent_tool_wrapper, true);
                let install_tool = match db.get_tool_by_key(&shinkai_tool.tool_router_key().to_string_without_version())
                {
                    Ok(_) => false,
                    Err(_) => true,
                };
                if install_tool {
                    db.add_tool(shinkai_tool).await.map_err(|e| APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Database Error".to_string(),
                        message: format!("Failed to save tool to database: {}", e),
                    })?;
                }

                let response = json!({
                    "status": "success",
                    "message": "Agent imported successfully",
                    "agent_id": agent.agent_id,
                    "agent": agent
                });
                return Ok(response);
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Database Error".to_string(),
                    message: format!("Failed to save agent to database: {}", err),
                };
                return Err(api_error);
            }
        }
    }

    import_agent_knowledge(zip_contents, db, embedding_generator).await?;

    return Ok(json!({
        "status": "success",
        "message": "Agent already installed",
        "agent_id": agent.agent_id,
        "agent": agent
    }));
}

pub fn get_tool_from_zip(mut archive: ZipArchive<std::io::Cursor<Vec<u8>>>) -> Result<ShinkaiTool, APIError> {
    // Extract and parse tool.json
    let mut buffer = Vec::new();
    {
        let mut file = match archive.by_name("__tool.json") {
            Ok(file) => file,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Tool Zip".to_string(),
                    message: "Archive does not contain __tool.json".to_string(),
                };
                return Err(api_error);
            }
        };

        if let Err(err) = file.read_to_end(&mut buffer) {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read tool.json: {}", err),
            };
            return Err(api_error);
        }
    }
    let tool: ShinkaiTool = serde_json::from_slice(&buffer).map_err(|e| APIError {
        code: StatusCode::BAD_REQUEST.as_u16(),
        error: "Invalid Tool JSON".to_string(),
        message: format!("Failed to parse tool.json: {}", e),
    })?;
    Ok(tool)
}

pub fn get_agent_from_zip(mut archive: ZipArchive<std::io::Cursor<Vec<u8>>>) -> Result<Agent, APIError> {
    // Extract and parse tool.json
    let mut buffer = Vec::new();
    {
        let mut file = match archive.by_name("__agent.json") {
            Ok(file) => file,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Agent Zip".to_string(),
                    message: "Archive does not contain __agent.json".to_string(),
                };
                return Err(api_error);
            }
        };

        if let Err(err) = file.read_to_end(&mut buffer) {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read agent.json: {}", err),
            };
            return Err(api_error);
        }
    }
    let agent: Agent = serde_json::from_slice(&buffer).map_err(|e| APIError {
        code: StatusCode::BAD_REQUEST.as_u16(),
        error: "Invalid Agent JSON".to_string(),
        message: format!("Failed to parse agent.json: {}", e),
    })?;
    Ok(agent)
}

async fn bytes_to_zip_tool(
    mut archive: ZipArchive<std::io::Cursor<Vec<u8>>>,
    file_name: String,
    is_tool: bool, // if not is agent
) -> Result<ZipFileContents, APIError> {
    // Extract and parse file
    let mut zip_buffer = Vec::new();
    {
        let mut file = match archive.by_name(&file_name) {
            Ok(file) => file,
            Err(_) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Zip File".to_string(),
                    message: format!("Archive does not contain {}", file_name),
                });
            }
        };

        // Read the file contents into a buffer
        if let Err(err) = file.read_to_end(&mut zip_buffer) {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Read Error".to_string(),
                message: format!("Failed to read file contents: {}", err),
            });
        }
    }

    // Create a new cursor and archive for returning
    let return_cursor = std::io::Cursor::new(zip_buffer.clone());
    let mut return_archive = zip::ZipArchive::new(return_cursor).unwrap();

    let mut tool_agent_buffer: Vec<u8> = Vec::new();
    {
        let mut file = match return_archive.by_name(if is_tool { "__tool.json" } else { "__agent.json" }) {
            Ok(file) => file,
            Err(_) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Zip File".to_string(),
                    message: "Archive does not contain __tool.json".to_string(),
                });
            }
        };

        if let Err(err) = file.read_to_end(&mut tool_agent_buffer) {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Invalid Tool JSON".to_string(),
                message: format!("Failed to read tool.json: {}", err),
            });
        }
    }

    Ok(ZipFileContents {
        buffer: tool_agent_buffer,
        archive: return_archive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::EmbeddingModelType;
    use shinkai_embedding::model_type::OllamaTextEmbeddingsInference;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
    use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
    use shinkai_tools_primitives::tools::agent_tool_wrapper::AgentToolWrapper;
    use shinkai_tools_primitives::tools::deno_tools::DenoTool;
    use shinkai_tools_primitives::tools::parameters::Parameters;
    use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
    use shinkai_tools_primitives::tools::tool_types::OperatingSystem;
    use shinkai_tools_primitives::tools::tool_types::RunnerType;
    use shinkai_tools_primitives::tools::tool_types::ToolResult;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);
        println!("Creating test db at {:?}", db_path);
        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_tool_dependency_cycles() {
        let manager = setup_test_db().await;
        let db = Arc::new(manager);
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        // Create three tools that form a cycle: A -> B -> C -> A
        let tool_a_name = "Tool A";
        let tool_a_version = "1.0.0";
        let tool_a_author = "@@test.shinkai";
        let mut tool_a = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_a_author.to_string(),
                name: tool_a_name.to_string(),
                version: Some(tool_a_version.to_string()),
            }),
            name: tool_a_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_a_author.to_string(),
            version: tool_a_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![], // A depends on B
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_b_name = "Tool B";
        let tool_b_version = "1.0.0";
        let tool_b_author = "@@test.shinkai";
        let mut tool_b = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_b_author.to_string(),
                name: tool_b_name.to_string(),
                version: Some(tool_b_version.to_string()),
            }),
            name: tool_b_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_b_author.to_string(),
            version: tool_b_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![], // B depends on C
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_c_name = "Tool C";
        let tool_c_version = "1.0.0";
        let tool_c_author = "@@test.shinkai";
        let mut tool_c = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_c_author.to_string(),
                name: tool_c_name.to_string(),
                version: Some(tool_c_version.to_string()),
            }),
            name: tool_c_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_c_author.to_string(),
            version: tool_c_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![], // C depends on A
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        tool_a
            .tools
            .push(ShinkaiTool::Deno(tool_b.clone(), true).tool_router_key());
        tool_b
            .tools
            .push(ShinkaiTool::Deno(tool_c.clone(), true).tool_router_key());
        tool_c
            .tools
            .push(ShinkaiTool::Deno(tool_a.clone(), true).tool_router_key());

        // Add tools to database
        db.add_tool_with_vector(
            ShinkaiTool::Deno(tool_a.clone(), true),
            SqliteManager::generate_vector_for_testing(0.1),
        )
        .unwrap();
        db.add_tool_with_vector(
            ShinkaiTool::Deno(tool_b.clone(), true),
            SqliteManager::generate_vector_for_testing(0.5),
        )
        .unwrap();
        db.add_tool_with_vector(
            ShinkaiTool::Deno(tool_c.clone(), true),
            SqliteManager::generate_vector_for_testing(0.9),
        )
        .unwrap();

        // Test calculate_zip_dependencies with tool A as entry point
        let mut agent_dependencies = HashMap::new();
        let mut tool_dependencies = HashMap::new();
        let result = calculate_zip_dependencies(
            db.clone(),
            profile.clone(),
            Some(ShinkaiTool::Deno(tool_a.clone(), true)),
            None,
            &mut agent_dependencies,
            &mut tool_dependencies,
        )
        .await;

        assert!(result.is_ok());
        assert!(tool_dependencies.len() == 3);
    }

    #[tokio::test]
    async fn test_tool_dependency_cycles_agent() {
        let manager = setup_test_db().await;
        let db = Arc::new(manager);
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let agent = Agent {
            name: "test_agent".to_string(),
            agent_id: "test123".to_string(),
            full_identity_name: ShinkaiName::new("test.agent".to_string()).unwrap(),
            llm_provider_id: "test_provider".to_string(),
            ui_description: "Test Agent".to_string(),
            knowledge: vec![],
            storage_path: "/test/path".to_string(),
            tools: vec![],
            debug_mode: false,
            config: None,
            cron_tasks: None,
            scope: MinimalJobScope::default(),
            tools_config_override: None,
        };

        let agent_tool_wrapper = ShinkaiTool::Agent(
            AgentToolWrapper::new(
                agent.agent_id.clone(),
                agent.name.clone(),
                agent.ui_description.clone(),
                profile.node_name.clone(),
                None,
            ),
            true,
        );
        // Create a tool that depends on an agent
        let tool_a_name = "Tool A";
        let tool_a_version = "1.0.0";
        let tool_a_author = "@@test.shinkai";
        let mut tool_a = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_a_author.to_string(),
                name: tool_a_name.to_string(),
                version: Some(tool_a_version.to_string()),
            }),
            name: tool_a_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_a_author.to_string(),
            version: tool_a_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![agent_tool_wrapper.tool_router_key()], // A depends on B
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_b_name = "Tool B";
        let tool_b_version = "1.0.0";
        let tool_b_author = "@@test.shinkai";
        let tool_b = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_b_author.to_string(),
                name: tool_b_name.to_string(),
                version: Some(tool_b_version.to_string()),
            }),
            name: tool_b_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_b_author.to_string(),
            version: tool_b_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![], // B depends on C
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        tool_a
            .tools
            .push(ShinkaiTool::Deno(tool_b.clone(), true).tool_router_key());

        db.add_agent(agent.clone(), &profile).unwrap();
        db.add_tool_with_vector(agent_tool_wrapper, SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        db.add_tool_with_vector(
            ShinkaiTool::Deno(tool_a.clone(), true),
            SqliteManager::generate_vector_for_testing(0.1),
        )
        .unwrap();
        db.add_tool_with_vector(
            ShinkaiTool::Deno(tool_b, true),
            SqliteManager::generate_vector_for_testing(0.5),
        )
        .unwrap();

        let mut agent_dependencies = HashMap::new();
        let mut tool_dependencies = HashMap::new();
        let result = calculate_zip_dependencies(
            db.clone(),
            profile.clone(),
            Some(ShinkaiTool::Deno(tool_a.clone(), true)),
            None,
            &mut agent_dependencies,
            &mut tool_dependencies,
        )
        .await;

        assert!(result.is_ok());
        assert!(agent_dependencies.len() == 1);
        assert!(tool_dependencies.len() == 3);
    }

    #[tokio::test]
    async fn test_agent_tool_dependencies() {
        let manager = setup_test_db().await;
        let db = Arc::new(manager);
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        // Create two tools that will be dependencies of the agent
        let tool_a_name = "Tool A";
        let tool_a_version = "1.0.0";
        let tool_a_author = "@@test.shinkai";
        let tool_a = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_a_author.to_string(),
                name: tool_a_name.to_string(),
                version: Some(tool_a_version.to_string()),
            }),
            name: tool_a_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_a_author.to_string(),
            version: tool_a_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_b_name = "Tool B";
        let tool_b_version = "1.0.0";
        let tool_b_author = "@@test.shinkai";
        let mut tool_b = DenoTool {
            tool_router_key: Some(ToolRouterKey {
                source: "local".to_string(),
                author: tool_b_author.to_string(),
                name: tool_b_name.to_string(),
                version: Some(tool_b_version.to_string()),
            }),
            name: tool_b_name.to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: tool_b_author.to_string(),
            version: tool_b_version.to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Add tools to database
        let tool_a_wrapper = ShinkaiTool::Deno(tool_a.clone(), true);
        let tool_b_wrapper = ShinkaiTool::Deno(tool_b.clone(), true);

        db.add_tool_with_vector(tool_a_wrapper.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        db.add_tool_with_vector(tool_b_wrapper.clone(), SqliteManager::generate_vector_for_testing(0.5))
            .unwrap();

        // Create an agent that depends on both tools
        let agent = Agent {
            name: "test_agent".to_string(),
            agent_id: "test123".to_string(),
            full_identity_name: ShinkaiName::new("test.agent".to_string()).unwrap(),
            llm_provider_id: "test_provider".to_string(),
            ui_description: "Test Agent".to_string(),
            knowledge: vec![],
            storage_path: "/test/path".to_string(),
            tools: vec![tool_a_wrapper.tool_router_key(), tool_b_wrapper.tool_router_key()],
            debug_mode: false,
            config: None,
            cron_tasks: None,
            scope: MinimalJobScope::default(),
            tools_config_override: None,
        };
        let agent_tool_wrapper = ShinkaiTool::Agent(
            AgentToolWrapper::new(
                agent.agent_id.clone(),
                agent.name.clone(),
                agent.ui_description.clone(),
                profile.node_name.clone(),
                None,
            ),
            true,
        );
        tool_b.tools.push(agent_tool_wrapper.tool_router_key());

        // Add agent to database
        db.add_agent(agent.clone(), &profile).unwrap();
        db.add_tool_with_vector(agent_tool_wrapper, SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        // Test calculate_zip_dependencies with agent as entry point
        let mut agent_dependencies = HashMap::new();
        let mut tool_dependencies = HashMap::new();
        let result = calculate_zip_dependencies(
            db.clone(),
            profile.clone(),
            None,
            Some(agent),
            &mut agent_dependencies,
            &mut tool_dependencies,
        )
        .await;

        assert!(result.is_ok());
        assert!(agent_dependencies.len() == 1);
        assert!(tool_dependencies.len() == 3);
    }
}
