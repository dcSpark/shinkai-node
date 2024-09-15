use crate::tools::error::ToolError;
use crate::tools::js_toolkit_headers::ToolConfig;
use crate::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use arrow_array::{Array, BinaryArray, BooleanArray};
use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field};
use futures::{StreamExt, TryStreamExt};
use lancedb::connection::LanceFileVersion;
use lancedb::index::scalar::{FtsIndexBuilder, FullTextSearchQuery};
use lancedb::index::vector::IvfHnswPqIndexBuilder;
use lancedb::index::Index;
use lancedb::query::QueryBase;
use lancedb::query::{ExecutableQuery, Select};
use lancedb::table::AddDataMode;
use lancedb::Error as LanceDbError;
use lancedb::{connect, Connection, Table};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::model_type::EmbeddingModelType;
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use std::time::Instant;

use super::ollama_embedding_fn::OllamaEmbeddingFunction;
use super::shinkai_lancedb_error::ShinkaiLanceDBError;
use super::shinkai_tool_schema::ShinkaiToolSchema;

// Note: Add 1 to the current number to force an old fashion migration (delete all and then add all)
pub static LATEST_ROUTER_DB_VERSION: &str = "5";

// TODO: we need a way to export and import the db (or tables). it could be much faster to reset.

#[derive(Clone)]
pub struct LanceShinkaiDb {
    #[allow(dead_code)]
    connection: Connection,
    pub tool_table: Table,
    pub version_table: Table,
    pub prompt_table: Table,
    pub embedding_model: EmbeddingModelType,
    pub embedding_function: OllamaEmbeddingFunction,
}

impl LanceShinkaiDb {
    pub async fn new(
        db_path: &str,
        embedding_model: EmbeddingModelType,
        generator: RemoteEmbeddingGenerator,
    ) -> Result<Self, ShinkaiLanceDBError> {
        let db_path = if db_path.starts_with("db") {
            eprintln!("Warning: db_path starts with 'db'. Prepending 'lance' to the path.");
            format!("lance{}", db_path)
        } else {
            db_path.to_string()
        };
        eprintln!("DB Path: {}", db_path);

        let connection = connect(&db_path).execute().await?;
        eprintln!("Connected to DB: {:?}", connection.uri());

        let version_table = Self::create_version_table(&connection).await?;
        eprintln!("Version table created");

        let tool_table = Self::create_tool_router_table(&connection, &embedding_model).await?;
        eprintln!("Tool table created");

        let prompt_table = Self::create_prompt_table(&connection, &embedding_model).await?;
        eprintln!("Prompt table created");

        let api_url = generator.api_url;
        let embedding_function = OllamaEmbeddingFunction::new(&api_url, embedding_model.clone());

        Ok(LanceShinkaiDb {
            connection,
            tool_table,
            version_table,
            prompt_table,
            embedding_model,
            embedding_function,
        })
    }

    async fn create_tool_router_table(
        connection: &Connection,
        embedding_model: &EmbeddingModelType,
    ) -> Result<Table, ShinkaiLanceDBError> {
        let schema = ShinkaiToolSchema::create_schema(embedding_model)
            .map_err(|e| ShinkaiLanceDBError::Schema(e.to_string()))?;

        let table = match connection
            .create_empty_table("tool_router_v5", schema)
            .data_storage_version(LanceFileVersion::V2_1)
            .enable_v2_manifest_paths(true)
            .execute()
            .await
        {
            Ok(table) => table,
            Err(e) => {
                if let LanceDbError::TableAlreadyExists { .. } = e {
                    // If the table already exists, retrieve and return it
                    connection
                        .open_table("tool_router_v5")
                        .execute()
                        .await
                        .map_err(ShinkaiLanceDBError::from)?
                } else {
                    return Err(ShinkaiLanceDBError::from(e));
                }
            }
        };

        Ok(table)
    }

    pub async fn create_tool_indices_if_needed(&self) -> Result<(), ShinkaiLanceDBError> {
        // Check if the table is empty
        let is_empty = Self::is_table_empty(&self.tool_table).await?;
        if is_empty {
            eprintln!("Tool Table is empty, skipping index creation.");
            return Ok(());
        }

        // Check the number of elements in the table
        let element_count = self.tool_table.count_rows(None).await?;
        if element_count < 100 {
            self.tool_table
                .create_index(&["tool_seo"], Index::FTS(FtsIndexBuilder::default()))
                .execute()
                .await?;

            eprintln!("Not enough elements to create other indices. Skipping index creation for tool table.");
            return Ok(());
        }

        // Create the indices
        self.tool_table
            .create_index(&["tool_key"], Index::Auto)
            .execute()
            .await?;

        self.tool_table
            .create_index(&["tool_seo"], Index::Auto)
            .execute()
            .await?;

        self.tool_table.create_index(&["vector"], Index::Auto).execute().await?;
        self.tool_table
            .create_index(
                &[ShinkaiToolSchema::vector_field()],
                Index::IvfHnswPq(IvfHnswPqIndexBuilder::default()),
            )
            .execute()
            .await?;

        Ok(())
    }

    pub async fn is_table_empty(table: &Table) -> Result<bool, ShinkaiLanceDBError> {
        let query = table.query().limit(1).execute().await?;
        let results = query.try_collect::<Vec<_>>().await?;
        Ok(results.is_empty())
    }

    /// Insert a tool into the database. It will overwrite the tool if it already exists.
    /// Also it auto-generates the embedding if it is not provided.
    pub async fn set_tool(&self, shinkai_tool: &ShinkaiTool) -> Result<(), ToolError> {
        let tool_key = shinkai_tool.tool_router_key().to_lowercase();
        let tool_keys = vec![shinkai_tool.tool_router_key()];
        let tool_seos = vec![shinkai_tool.format_embedding_string()];
        let tool_types = vec![shinkai_tool.tool_type().to_string()];

        // Check if the tool already exists and delete it if it does
        if self
            .tool_exists(&tool_key)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
        {
            self.tool_table
                .delete(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key).as_str())
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        // Get or generate the embedding
        let embedding = match shinkai_tool.get_embedding() {
            Some(embedding) => embedding.vector,
            None => {
                eprintln!("Generating embedding for tool: {}", tool_key);
                let embedding_string = shinkai_tool.format_embedding_string();
                self.embedding_function
                    .request_embeddings(&embedding_string)
                    .await
                    .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?
            }
        };
        let vectors = embedding;

        // Update the tool header and data if the tool cannot be enabled
        let mut shinkai_tool = shinkai_tool.clone();

        // For Debugging
        // add an if using env REINSTALL_TOOLS so we inject some configuration data for certain tools and also we make it enabled
        if env::var("REINSTALL_TOOLS").is_ok() || env::var("INITIAL_CONF").is_ok() {
            if let ShinkaiTool::JS(ref mut js_tool, _) = shinkai_tool {
                if tool_key.starts_with("local:::shinkai-tool-coinbase") {
                    if let (Ok(api_name), Ok(private_key), Ok(wallet_id), Ok(use_server_signer)) = (
                        env::var("COINBASE_API_NAME"),
                        env::var("COINBASE_API_PRIVATE_KEY"),
                        env::var("COINBASE_API_WALLET_ID"),
                        env::var("COINBASE_API_USE_SERVER_SIGNER"),
                    ) {
                        for config in &mut js_tool.config {
                            if let ToolConfig::BasicConfig(ref mut basic_config) = config {
                                if basic_config.key_name == "name" {
                                    basic_config.key_value = Some(api_name.clone());
                                } else if basic_config.key_name == "privateKey" {
                                    basic_config.key_value = Some(private_key.clone());
                                } else if basic_config.key_name == "walletId" {
                                    basic_config.key_value = Some(wallet_id.clone());
                                } else if basic_config.key_name == "useServerSigner" {
                                    basic_config.key_value = Some(use_server_signer.clone());
                                }
                            }
                        }
                        shinkai_tool.enable();
                    }
                } else if tool_key.starts_with("local:::shinkai-tool-youtube-transcript") {
                    if let (Ok(api_url), Ok(api_key), Ok(model)) = (
                        env::var("YOUTUBE_TRANSCRIPT_API_URL"),
                        env::var("YOUTUBE_TRANSCRIPT_API_KEY"),
                        env::var("YOUTUBE_TRANSCRIPT_API_MODEL"),
                    ) {
                        for config in &mut js_tool.config {
                            if let ToolConfig::BasicConfig(ref mut basic_config) = config {
                                if basic_config.key_name == "apiUrl" {
                                    basic_config.key_value = Some(api_url.clone());
                                } else if basic_config.key_name == "apiKey" {
                                    basic_config.key_value = Some(api_key.clone());
                                } else if basic_config.key_name == "model" {
                                    basic_config.key_value = Some(model.clone());
                                }
                            }
                        }
                        shinkai_tool.enable();
                    }
                }
            }
        }

        let is_enabled = match shinkai_tool.is_enabled() {
            true => shinkai_tool.can_be_enabled(),
            false => false,
        };

        if shinkai_tool.is_enabled() && !shinkai_tool.can_be_enabled() {
            shinkai_tool.disable();
        }

        let tool_data_vec =
            serde_json::to_vec(&shinkai_tool).map_err(|e| ToolError::SerializationError(e.to_string()))?;
        let tool_data: Vec<&[u8]> = vec![tool_data_vec.as_slice()];

        let tool_header = vec![serde_json::to_string(&shinkai_tool.to_header())
            .map_err(|e| ToolError::SerializationError(e.to_string()))?];

        let schema = self
            .tool_table
            .schema()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let vector_dimensions = self
            .embedding_model
            .vector_dimensions()
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let vectors_normalized = Arc::new(Float32Array::from(vectors));

        // Extract on_demand_price and is_network
        let (on_demand_price, is_network) = match shinkai_tool {
            ShinkaiTool::Network(ref network_tool, _) => {
                let price = network_tool.usage_type.per_use_usd_price();
                (Some(price), true)
            }
            _ => (None, false),
        };

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(tool_keys)),
                Arc::new(StringArray::from(tool_seos)),
                Arc::new(
                    FixedSizeListArray::try_new(
                        Arc::new(Field::new("item", DataType::Float32, true)),
                        vector_dimensions.try_into().unwrap(),
                        vectors_normalized,
                        None,
                    )
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?,
                ),
                Arc::new(StringArray::from(tool_types)),
                Arc::new(BinaryArray::from(tool_data)),
                Arc::new(StringArray::from(tool_header)),
                Arc::new(StringArray::from(vec![shinkai_tool.author().to_string()])),
                Arc::new(StringArray::from(vec![shinkai_tool.version().to_string()])),
                Arc::new(BooleanArray::from(vec![is_enabled])),
                Arc::new(
                    on_demand_price
                        .map_or_else(|| Float32Array::from(vec![None]), |p| Float32Array::from(vec![Some(p)])),
                ),
                Arc::new(BooleanArray::from(vec![is_network])),
            ],
        )
        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let batch_reader = Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema.clone()));

        self.tool_table
            .add(batch_reader)
            .mode(AddDataMode::Append)
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_tool(&self, tool_key: &str) -> Result<Option<ShinkaiTool>, ShinkaiLanceDBError> {
        let start_time = Instant::now();

        let query = self
            .tool_table
            .query()
            .only_if(format!(
                "{} = '{}'",
                ShinkaiToolSchema::tool_key_field(),
                tool_key.to_lowercase()
            ))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut res = query;
        while let Some(Ok(batch)) = res.next().await {
            let tool_data_array = batch
                .column_by_name(ShinkaiToolSchema::tool_data_field())
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap();

            if tool_data_array.len() > 0 {
                let tool_data = tool_data_array.value(0);
                let shinkai_tool: ShinkaiTool =
                    serde_json::from_slice(tool_data).map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                let duration = start_time.elapsed();
                println!("Time taken to fetch tool with key '{}': {:?}", tool_key, duration);
                return Ok(Some(shinkai_tool));
            }
        }
        Ok(None)
    }

    pub async fn remove_tool(&self, tool_key: &str) -> Result<(), ShinkaiLanceDBError> {
        self.tool_table
            .delete(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key).as_str())
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))
    }

    pub async fn tool_exists(&self, tool_key: &str) -> Result<bool, ShinkaiLanceDBError> {
        let query = self
            .tool_table
            .query()
            .only_if(format!(
                "{} = '{}'",
                ShinkaiToolSchema::tool_key_field(),
                tool_key.to_lowercase()
            ))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        Ok(!results.is_empty())
    }

    pub async fn get_all_workflows(&self) -> Result<Vec<ShinkaiToolHeader>, ShinkaiLanceDBError> {
        let query = self
            .tool_table
            .query()
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .only_if(format!("{} = 'Workflow'", ShinkaiToolSchema::tool_type_field()))
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut res = query;
        let mut workflows = Vec::new();

        while let Some(Ok(batch)) = res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                workflows.push(tool_header);
            }
        }
        Ok(workflows)
    }

    pub async fn get_all_tools(
        &self,
        include_network_tools: bool,
    ) -> Result<Vec<ShinkaiToolHeader>, ShinkaiLanceDBError> {
        let mut query_builder = self.tool_table.query().select(Select::columns(&[
            ShinkaiToolSchema::tool_key_field(),
            ShinkaiToolSchema::tool_type_field(),
            ShinkaiToolSchema::tool_header_field(),
        ]));

        if !include_network_tools {
            query_builder = query_builder.only_if(format!("{} = false", ShinkaiToolSchema::is_network_field()));
        }

        let query = query_builder
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut res = query;
        let mut tools = Vec::new();

        while let Some(Ok(batch)) = res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                tools.push(tool_header);
            }
        }
        Ok(tools)
    }

    pub async fn vector_search_enabled_tools(
        &self,
        query: &str,
        num_results: u64,
        include_network_tools: bool,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let filter = if include_network_tools {
            Some(format!("{} = true", ShinkaiToolSchema::is_enabled_field()))
        } else {
            Some(format!(
                "{} = true AND {} = false",
                ShinkaiToolSchema::is_enabled_field(),
                ShinkaiToolSchema::is_network_field()
            ))
        };

        self.vector_search_tools(query, num_results, filter.as_deref()).await
    }

    pub async fn vector_search_all_tools(
        &self,
        query: &str,
        num_results: u64,
        include_network_tools: bool,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let filter = if include_network_tools {
            None
        } else {
            Some(format!("{} = false", ShinkaiToolSchema::is_network_field()))
        };

        self.vector_search_tools(query, num_results, filter.as_deref()).await
    }

    async fn vector_search_tools(
        &self,
        query: &str,
        num_results: u64,
        filter: Option<&str>,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let vector_query = self
            .embedding_function
            .request_embeddings(query)
            .await
            .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?;

        // Full-text search query
        let mut fts_query_builder = self
            .tool_table
            .query()
            .full_text_search(FullTextSearchQuery::new(query.to_owned()))
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .limit(num_results as usize);

        // Vector search query
        let mut vector_query_builder = self
            .tool_table
            .query()
            .nearest_to(vector_query)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .limit(num_results as usize);

        if let Some(filter) = filter {
            fts_query_builder = fts_query_builder.only_if(filter.to_string());
            vector_query_builder = vector_query_builder.only_if(filter.to_string());
        }

        // Execute the full-text search
        let fts_query = fts_query_builder
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Execute the vector search
        let vector_query = vector_query_builder
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        // Collect results from both queries
        let mut fts_results = Vec::new();
        let mut vector_results = Vec::new();

        let mut fts_res = fts_query;
        while let Some(Ok(batch)) = fts_res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ToolError::SerializationError(e.to_string()))?;
                fts_results.push(tool_header);
            }
        }

        let mut vector_res = vector_query;
        while let Some(Ok(batch)) = vector_res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ToolError::SerializationError(e.to_string()))?;
                vector_results.push(tool_header);
            }
        }

        // Merge results using interleave and remove duplicates
        let mut combined_results = Vec::new();
        let mut seen = HashSet::new();
        let mut fts_iter = fts_results.into_iter();
        let mut vector_iter = vector_results.into_iter();

        while combined_results.len() < num_results as usize {
            match (fts_iter.next(), vector_iter.next()) {
                (Some(fts_item), Some(vector_item)) => {
                    if seen.insert(fts_item.tool_router_key.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                    if seen.insert(vector_item.tool_router_key.clone()) && combined_results.len() < num_results as usize
                    {
                        combined_results.push(vector_item);
                    }
                }
                (Some(fts_item), None) => {
                    if seen.insert(fts_item.tool_router_key.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                }
                (None, Some(vector_item)) => {
                    if seen.insert(vector_item.tool_router_key.clone()) && combined_results.len() < num_results as usize
                    {
                        combined_results.push(vector_item);
                    }
                }
                (None, None) => break,
            }
        }

        Ok(combined_results)
    }

    pub async fn workflow_vector_search(
        &self,
        query: &str,
        num_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let embedding = self
            .embedding_function
            .request_embeddings(query)
            .await
            .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?;

        // Full-text search query
        let mut fts_query_builder = self
            .tool_table
            .query()
            .full_text_search(FullTextSearchQuery::new(query.to_owned()))
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .only_if(format!(
                "{} = 'Workflow' AND {} = true",
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::is_enabled_field()
            ))
            .limit(num_results as usize);

        // Vector search query
        let mut vector_query_builder = self
            .tool_table
            .query()
            .nearest_to(embedding)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .only_if(format!(
                "{} = 'Workflow' AND {} = true",
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::is_enabled_field()
            ))
            .limit(num_results as usize);

        // Execute the full-text search
        let fts_query = fts_query_builder
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Execute the vector search
        let vector_query = vector_query_builder
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        // Collect results from both queries
        let mut fts_results = Vec::new();
        let mut vector_results = Vec::new();

        let mut fts_res = fts_query;
        while let Some(Ok(batch)) = fts_res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ToolError::SerializationError(e.to_string()))?;
                fts_results.push(tool_header);
            }
        }

        let mut vector_res = vector_query;
        while let Some(Ok(batch)) = vector_res.next().await {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ToolError::SerializationError(e.to_string()))?;
                vector_results.push(tool_header);
            }
        }

        // Merge results using interleave and remove duplicates
        let mut combined_results = Vec::new();
        let mut seen = HashSet::new();
        let mut fts_iter = fts_results.into_iter();
        let mut vector_iter = vector_results.into_iter();

        while combined_results.len() < num_results as usize {
            match (fts_iter.next(), vector_iter.next()) {
                (Some(fts_item), Some(vector_item)) => {
                    if seen.insert(fts_item.tool_router_key.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                    if seen.insert(vector_item.tool_router_key.clone()) && combined_results.len() < num_results as usize
                    {
                        combined_results.push(vector_item);
                    }
                }
                (Some(fts_item), None) => {
                    if seen.insert(fts_item.tool_router_key.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                }
                (None, Some(vector_item)) => {
                    if seen.insert(vector_item.tool_router_key.clone()) && combined_results.len() < num_results as usize
                    {
                        combined_results.push(vector_item);
                    }
                }
                (None, None) => break,
            }
        }

        Ok(combined_results)
    }

    pub async fn is_empty(&self) -> Result<bool, ShinkaiLanceDBError> {
        let query = self
            .tool_table
            .query()
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        Ok(results.is_empty())
    }

    pub async fn has_any_js_tools(&self) -> Result<bool, ShinkaiLanceDBError> {
        let query = self
            .tool_table
            .query()
            .only_if(format!("{} = 'JS'", ShinkaiToolSchema::tool_type_field()))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        Ok(!results.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use crate::network::agent_payments_manager::shinkai_tool_offering::ToolPrice;
    use crate::network::agent_payments_manager::shinkai_tool_offering::UsageType;
    use crate::tools::js_toolkit::JSToolkit;
    use crate::tools::js_toolkit_headers::ToolConfig;
    use crate::tools::network_tool::NetworkTool;
    use crate::tools::tool_router_dep::workflows_data;

    use super::*;
    use serde_json::Value;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_tools_runner::built_in_tools;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use std::fs;
    use std::path::Path;
    use std::time::Instant;

    fn setup() {
        let path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_vector_search_and_basics() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let tools = built_in_tools::get_tools();

        // Start the timer
        let start_time = Instant::now();

        // Install built-in toolkits
        let mut tool_count = 0;

        // Install built-in toolkits
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
                let embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                shinkai_tool.set_embedding(embedding);

                db.set_tool(&shinkai_tool)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                tool_count += 1;
            }
        }

        db.create_tool_indices_if_needed().await?;

        // Stop the timer
        let duration = start_time.elapsed();
        println!("Added {} tools in {:?}", tool_count, duration);

        let query = "duckduckgo";
        let results = db
            .vector_search_enabled_tools(&query, 5, false)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut found_duckduckgo = false;
        let mut duckduckgo_tool_key = String::new();

        for tool_header in results {
            let tool_key = tool_header.tool_router_key.clone();
            println!("Tool key: {}", tool_key);
            if tool_key.contains("duckduckgo") {
                found_duckduckgo = true;
                duckduckgo_tool_key = tool_key;
                eprintln!("Found duckduckgo tool key: {}", duckduckgo_tool_key);
            }
        }

        assert!(found_duckduckgo, "duckduckgo not found in the results");

        // Use get_tool to fetch the tool and check that they match
        let fetched_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(fetched_tool.is_some(), "Failed to fetch the tool using get_tool");
        let mut fetched_tool = fetched_tool.unwrap();
        assert_eq!(
            fetched_tool.tool_router_key(),
            duckduckgo_tool_key,
            "Tool keys do not match"
        );

        // Print the author name before the change
        if let ShinkaiTool::JS(ref js_tool, _) = fetched_tool {
            println!("Author before change: {}", js_tool.author);
        }

        // Update the author name of the tool
        if let ShinkaiTool::JS(ref mut js_tool, _) = fetched_tool {
            js_tool.author = "New Author".to_string();
        }

        // Update the tool in the database
        db.set_tool(&fetched_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Re-fetch the tool and check the updated author name
        let updated_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(updated_tool.is_some(), "Failed to fetch the updated tool");
        let updated_tool = updated_tool.unwrap();
        if let ShinkaiTool::JS(js_tool, _) = updated_tool {
            assert_eq!(js_tool.author, "New Author", "Author name was not updated");
        }

        // Delete the tool from the database
        db.remove_tool(&duckduckgo_tool_key)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Try to fetch the tool again and ensure it is not available
        let deleted_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(deleted_tool.is_none(), "Tool was not deleted successfully");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_tools_and_workflows() -> Result<(), ShinkaiLanceDBError> {
        setup();

        // Set the environment variable to enable testing workflows
        // env::set_var("IS_TESTING", "true");

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let tools = built_in_tools::get_tools();

        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
                let embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                shinkai_tool.set_embedding(embedding);

                db.set_tool(&shinkai_tool)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
            }
        }

        db.create_tool_indices_if_needed().await?;

        // Start the timer
        let start_time = Instant::now();
        // Install built-in toolkits
        let mut tool_count = 0;
        // Install workflows
        let data = workflows_data::WORKFLOWS_JSON; // WORKFLOWS_JSON_TESTING
        let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
        let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        for item in json_array {
            let shinkai_tool: Result<ShinkaiTool, _> = serde_json::from_value(item.clone());
            let shinkai_tool = match shinkai_tool {
                Ok(tool) => tool,
                Err(e) => {
                    eprintln!("Failed to parse shinkai_tool: {}. JSON: {:?}", e, item);
                    continue; // Skip this item and continue with the next one
                }
            };

            db.set_tool(&shinkai_tool)
                .await
                .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
            tool_count += 1;
        }

        // Stop the timer
        let duration = start_time.elapsed();
        println!("Added {} workflows in {:?}", tool_count, duration);

        let query = "search";

        // Start the search timer
        let search_start_time = Instant::now();

        let results = db
            .vector_search_enabled_tools(&query, 2, false)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // eprintln!("Workflow Results: {:?}", results);

        // Stop the search timer
        let search_duration = search_start_time.elapsed();
        println!("Search took {:?}", search_duration);

        assert!(!results.is_empty(), "No results found for the query");

        // Measure get_all_workflows execution time
        let workflows_start_time = Instant::now();

        let workflows = db.get_all_workflows().await?;
        // eprintln!("Workflows: {:?}", workflows);
        let workflows_duration = workflows_start_time.elapsed();
        println!("get_all_workflows took {:?}", workflows_duration);

        assert!(!workflows.is_empty(), "No workflows found");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_tool_and_update_config() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let tools = built_in_tools::get_tools();
        let (name, definition) = tools
            .into_iter()
            .find(|(name, _)| name == "shinkai-tool-weather-by-city")
            .unwrap();

        let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
        let tool = toolkit.tools.into_iter().next().unwrap();
        let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
        let embedding = generator
            .generate_embedding_default(&shinkai_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_tool.set_embedding(embedding);

        db.set_tool(&shinkai_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Read the config (should be null)
        let tool_key = shinkai_tool.tool_router_key();
        let fetched_tool = db.get_tool(&tool_key).await?;
        assert!(fetched_tool.is_some(), "Failed to fetch the tool using get_tool");
        let mut fetched_tool = fetched_tool.unwrap();
        if let ShinkaiTool::JS(ref js_tool, _) = fetched_tool {
            for config in &js_tool.config {
                if let ToolConfig::BasicConfig(ref basic_config) = config {
                    assert!(basic_config.key_value.is_none(), "Initial key_value should be None");
                }
            }
        }

        // Update the config with a random JSON value
        let new_config_value = "new_value".to_string();
        if let ShinkaiTool::JS(ref mut js_tool, _) = fetched_tool {
            for config in &mut js_tool.config {
                if let ToolConfig::BasicConfig(ref mut basic_config) = config {
                    basic_config.key_value = Some(new_config_value.clone());
                }
            }
        }

        // Update the tool in the database
        db.set_tool(&fetched_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Re-fetch the tool and ensure the config matches
        let updated_tool = db.get_tool(&tool_key).await?;
        assert!(updated_tool.is_some(), "Failed to fetch the updated tool");
        let updated_tool = updated_tool.unwrap();
        if let ShinkaiTool::JS(js_tool, _) = updated_tool {
            for config in &js_tool.config {
                if let ToolConfig::BasicConfig(basic_config) = config {
                    assert_eq!(
                        basic_config.key_value,
                        Some(new_config_value.clone()),
                        "Config values do not match"
                    );
                }
            }
        } else {
            assert!(false, "Updated tool is not of type JS");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_add_workflow_and_js_tool() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        // Add JS tool
        let tools = built_in_tools::get_tools();
        let (name, definition) = tools
            .into_iter()
            .find(|(name, _)| name == "shinkai-tool-weather-by-city")
            .unwrap();

        let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
        let tool = toolkit.tools.into_iter().next().unwrap();
        let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
        let embedding = generator
            .generate_embedding_default(&shinkai_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_tool.set_embedding(embedding);

        db.set_tool(&shinkai_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Add workflow
        let data = workflows_data::WORKFLOWS_JSON_TESTING;
        let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
        let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        for item in json_array {
            let shinkai_tool: Result<ShinkaiTool, _> = serde_json::from_value(item.clone());
            let shinkai_tool = match shinkai_tool {
                Ok(tool) => tool,
                Err(e) => {
                    eprintln!("Failed to parse shinkai_tool: {}. JSON: {:?}", e, item);
                    continue; // Skip this item and continue with the next one
                }
            };

            db.set_tool(&shinkai_tool)
                .await
                .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        }

        // Verify both tools are added
        let all_tools = db.get_all_tools(false).await?;
        let mut found_js_tool = false;
        let mut found_workflow = false;

        for tool_header in all_tools {
            if tool_header.tool_type == "JS" {
                found_js_tool = true;
            } else if tool_header.tool_type == "Workflow" {
                found_workflow = true;
            }
        }

        assert!(found_js_tool, "JS tool not found");
        assert!(found_workflow, "Workflow not found");

        Ok(())
    }

    #[tokio::test]
    async fn test_has_any_js_tools() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        // Initially, the database should be empty for JS tools
        assert!(
            !db.has_any_js_tools().await?,
            "Database should be empty for JS tools initially"
        );

        // Add a JS tool
        let tools = built_in_tools::get_tools();
        let (name, definition) = tools
            .into_iter()
            .find(|(name, _)| name == "shinkai-tool-weather-by-city")
            .unwrap();

        let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
        let tool = toolkit.tools.into_iter().next().unwrap();
        let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
        let embedding = generator
            .generate_embedding_default(&shinkai_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_tool.set_embedding(embedding);

        db.set_tool(&shinkai_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Now, the database should not be empty for JS tools
        assert!(
            db.has_any_js_tools().await?,
            "Database should not be empty for JS tools after adding one"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_add_js_and_network_tools() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        // Add JS tool
        let tools = built_in_tools::get_tools();
        let (js_name, js_definition) = tools
            .iter()
            .find(|(name, _)| *name == "shinkai-tool-weather-by-city")
            .unwrap();

        let js_toolkit = JSToolkit::new(js_name, vec![js_definition.clone()]);
        let js_tool = js_toolkit.tools.into_iter().next().unwrap();
        let mut shinkai_js_tool = ShinkaiTool::JS(js_tool.clone(), true);
        let js_embedding = generator
            .generate_embedding_default(&shinkai_js_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_js_tool.set_embedding(js_embedding);

        db.set_tool(&shinkai_js_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Add Network tool
        let network_tool = NetworkTool {
            name: "network_tool_example".to_string(),
            toolkit_name: "shinkai-tool-echo".to_string(),
            description: "A network tool example".to_string(),
            version: "1.0".to_string(),
            provider: ShinkaiName::new("@@nico.shinkai".to_string()).unwrap(),
            usage_type: UsageType::PerUse(ToolPrice::Free),
            activated: true,
            config: vec![],
            input_args: vec![],
            embedding: None,
            restrictions: None,
        };
        let mut shinkai_network_tool = ShinkaiTool::Network(network_tool.clone(), true);
        let network_embedding = generator
            .generate_embedding_default(&shinkai_network_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_network_tool.set_embedding(network_embedding);

        db.set_tool(&shinkai_network_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        db.create_tool_indices_if_needed().await?;

        // Test get_all_tools with include_network_tools = true
        let all_tools_with_network = db.get_all_tools(true).await?;
        assert!(
            all_tools_with_network.iter().any(|tool| tool.tool_type == "JS"),
            "JS tool not found"
        );
        assert!(
            all_tools_with_network.iter().any(|tool| tool.tool_type == "Network"),
            "Network tool not found"
        );

        // Test get_all_tools with include_network_tools = false
        let all_tools_without_network = db.get_all_tools(false).await?;
        assert!(
            all_tools_without_network.iter().any(|tool| tool.tool_type == "JS"),
            "JS tool not found"
        );
        assert!(
            !all_tools_without_network.iter().any(|tool| tool.tool_type == "Network"),
            "Network tool should not be found"
        );

        // Test vector_search_all_tools with include_network_tools = true
        let query = "example";
        let search_results_with_network = db
            .vector_search_all_tools(query, 5, true)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        assert!(
            search_results_with_network.iter().any(|tool| tool.tool_type == "JS"),
            "JS tool not found in search results"
        );
        assert!(
            search_results_with_network
                .iter()
                .any(|tool| tool.tool_type == "Network"),
            "Network tool not found in search results"
        );

        // Test vector_search_all_tools with include_network_tools = false
        let search_results_without_network = db
            .vector_search_all_tools(query, 5, false)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        assert!(
            search_results_without_network.iter().any(|tool| tool.tool_type == "JS"),
            "JS tool not found in search results"
        );
        assert!(
            !search_results_without_network
                .iter()
                .any(|tool| tool.tool_type == "Network"),
            "Network tool should not be found in search results"
        );

        Ok(())
    }
}
