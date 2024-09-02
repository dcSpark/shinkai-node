use polars::{io::SerReader, prelude::ParquetReader};
use shinkai_graphrag::{
    indexer_adapters::{
        read_indexer_entities, read_indexer_relationships, read_indexer_reports, read_indexer_text_units,
    },
    input::loaders::dfs::store_entity_semantic_embeddings,
    llm::base::LLMParams,
    search::local_search::{
        mixed_context::{default_local_context_params, LocalSearchMixedContext},
        search::LocalSearch,
    },
    vector_stores::lancedb::LanceDBVectorStore,
};
use utils::{
    ollama::OllamaChat,
    openai::{num_tokens, ChatOpenAI, OpenAIEmbedding},
};

mod utils;

#[tokio::test]
async fn ollama_local_search_test() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = "http://localhost:11434";
    let llm_model = "llama3.1";
    let llm = OllamaChat::new(base_url, llm_model);

    // Using OpenAI embeddings since the dataset was created with OpenAI embeddings
    let api_key = std::env::var("GRAPHRAG_API_KEY").unwrap();
    let embedding_model = std::env::var("GRAPHRAG_EMBEDDING_MODEL").unwrap();
    let text_embedder = OpenAIEmbedding::new(Some(api_key), &embedding_model, 8191, 5);

    // Load community reports
    // Download dataset: https://microsoft.github.io/graphrag/data/operation_dulce/dataset.zip

    let input_dir = "./dataset";
    let lancedb_uri = format!("{}/lancedb", input_dir);

    let community_report_table = "create_final_community_reports";
    let entity_table = "create_final_nodes";
    let entity_embedding_table = "create_final_entities";
    let relationship_table = "create_final_relationships";
    let text_unit_table = "create_final_text_units";
    let community_level = 2;

    // Read entities
    let mut entity_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_table)).unwrap();
    let entity_df = ParquetReader::new(&mut entity_file).finish().unwrap();

    let mut entity_embedding_file =
        std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_embedding_table)).unwrap();
    let entity_embedding_df = ParquetReader::new(&mut entity_embedding_file).finish().unwrap();

    let entities = read_indexer_entities(&entity_df, &entity_embedding_df, community_level)?;

    let mut description_embedding_store = LanceDBVectorStore::new("entity_description_embeddings".to_string());
    description_embedding_store.connect(&lancedb_uri).await?;

    store_entity_semantic_embeddings(entities.clone(), &mut description_embedding_store).await?;

    println!("Entities ({}): {:?}", entity_df.height(), entity_df.head(Some(5)));

    // Read relationships
    let mut relationship_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, relationship_table)).unwrap();
    let relationship_df = ParquetReader::new(&mut relationship_file).finish().unwrap();

    let relationships = read_indexer_relationships(&relationship_df)?;

    println!(
        "Relationships ({}): {:?}",
        relationship_df.height(),
        relationship_df.head(Some(5))
    );

    // Read community reports
    let mut report_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, community_report_table)).unwrap();
    let report_df = ParquetReader::new(&mut report_file).finish().unwrap();

    let reports = read_indexer_reports(&report_df, &entity_df, community_level)?;

    println!("Reports ({}): {:?}", report_df.height(), report_df.head(Some(5)));

    // Read text units
    let mut text_unit_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, text_unit_table)).unwrap();
    let text_unit_df = ParquetReader::new(&mut text_unit_file).finish().unwrap();

    let text_units = read_indexer_text_units(&text_unit_df)?;

    println!(
        "Text units ({}): {:?}",
        text_unit_df.height(),
        text_unit_df.head(Some(5))
    );

    // Create local search context builder
    let context_builder = LocalSearchMixedContext::new(
        entities,
        description_embedding_store,
        Box::new(text_embedder),
        Some(text_units),
        Some(reports),
        Some(relationships),
        num_tokens,
        "id".to_string(),
    );

    // Create local search engine
    let mut local_context_params = default_local_context_params();
    local_context_params.text_unit_prop = 0.5;
    local_context_params.community_prop = 0.1;
    local_context_params.top_k_mapped_entities = 10;
    local_context_params.top_k_relationships = 10;
    local_context_params.include_entity_rank = true;
    local_context_params.include_relationship_weight = true;
    local_context_params.include_community_rank = false;
    local_context_params.return_candidate_context = false;
    local_context_params.max_tokens = 12_000;

    let llm_params = LLMParams {
        max_tokens: 2000,
        temperature: 0.0,
    };

    let search_engine = LocalSearch::new(
        Box::new(llm),
        context_builder,
        num_tokens,
        llm_params,
        local_context_params,
        String::from("multiple paragraphs"),
        None,
    );

    let result = search_engine.asearch("Tell me about Agent Mercer".to_string()).await?;
    println!("Response: {:?}\n", result.response);

    let result = search_engine
        .asearch("Tell me about Dr. Jordan Hayes".to_string())
        .await?;
    println!("Response: {:?}\n", result.response);

    match result.context_data {
        shinkai_graphrag::search::base::ContextData::Dictionary(dict) => {
            for (entity, df) in dict.iter() {
                println!("Data: {} ({})", entity, df.height());
                println!("{:?}", df.head(Some(10)));
            }
        }
        data => {
            println!("Context data: {:?}", data);
        }
    }

    Ok(())
}

#[tokio::test]
async fn openai_local_search_test() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GRAPHRAG_API_KEY").unwrap();
    let llm_model = std::env::var("GRAPHRAG_LLM_MODEL").unwrap();
    let embedding_model = std::env::var("GRAPHRAG_EMBEDDING_MODEL").unwrap();

    let llm = ChatOpenAI::new(Some(api_key.clone()), &llm_model, 5);
    let text_embedder = OpenAIEmbedding::new(Some(api_key), &embedding_model, 8191, 5);

    // Load community reports
    // Download dataset: https://microsoft.github.io/graphrag/data/operation_dulce/dataset.zip

    let input_dir = "./dataset";
    let lancedb_uri = format!("{}/lancedb", input_dir);

    let community_report_table = "create_final_community_reports";
    let entity_table = "create_final_nodes";
    let entity_embedding_table = "create_final_entities";
    let relationship_table = "create_final_relationships";
    let text_unit_table = "create_final_text_units";
    let community_level = 2;

    // Read entities
    let mut entity_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_table)).unwrap();
    let entity_df = ParquetReader::new(&mut entity_file).finish().unwrap();

    let mut entity_embedding_file =
        std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_embedding_table)).unwrap();
    let entity_embedding_df = ParquetReader::new(&mut entity_embedding_file).finish().unwrap();

    let entities = read_indexer_entities(&entity_df, &entity_embedding_df, community_level)?;

    let mut description_embedding_store = LanceDBVectorStore::new("entity_description_embeddings".to_string());
    description_embedding_store.connect(&lancedb_uri).await?;

    store_entity_semantic_embeddings(entities.clone(), &mut description_embedding_store).await?;

    println!("Entities ({}): {:?}", entity_df.height(), entity_df.head(Some(5)));

    // Read relationships
    let mut relationship_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, relationship_table)).unwrap();
    let relationship_df = ParquetReader::new(&mut relationship_file).finish().unwrap();

    let relationships = read_indexer_relationships(&relationship_df)?;

    println!(
        "Relationships ({}): {:?}",
        relationship_df.height(),
        relationship_df.head(Some(5))
    );

    // Read community reports
    let mut report_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, community_report_table)).unwrap();
    let report_df = ParquetReader::new(&mut report_file).finish().unwrap();

    let reports = read_indexer_reports(&report_df, &entity_df, community_level)?;

    println!("Reports ({}): {:?}", report_df.height(), report_df.head(Some(5)));

    // Read text units
    let mut text_unit_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, text_unit_table)).unwrap();
    let text_unit_df = ParquetReader::new(&mut text_unit_file).finish().unwrap();

    let text_units = read_indexer_text_units(&text_unit_df)?;

    println!(
        "Text units ({}): {:?}",
        text_unit_df.height(),
        text_unit_df.head(Some(5))
    );

    // Create local search context builder
    let context_builder = LocalSearchMixedContext::new(
        entities,
        description_embedding_store,
        Box::new(text_embedder),
        Some(text_units),
        Some(reports),
        Some(relationships),
        num_tokens,
        "id".to_string(),
    );

    // Create local search engine
    let mut local_context_params = default_local_context_params();
    local_context_params.text_unit_prop = 0.5;
    local_context_params.community_prop = 0.1;
    local_context_params.top_k_mapped_entities = 10;
    local_context_params.top_k_relationships = 10;
    local_context_params.include_entity_rank = true;
    local_context_params.include_relationship_weight = true;
    local_context_params.include_community_rank = false;
    local_context_params.return_candidate_context = false;
    local_context_params.max_tokens = 12_000;

    let llm_params = LLMParams {
        max_tokens: 2000,
        temperature: 0.0,
    };

    let search_engine = LocalSearch::new(
        Box::new(llm),
        context_builder,
        num_tokens,
        llm_params,
        local_context_params,
        String::from("multiple paragraphs"),
        None,
    );

    let result = search_engine.asearch("Tell me about Agent Mercer".to_string()).await?;
    println!("Response: {:?}\n", result.response);

    let result = search_engine
        .asearch("Tell me about Dr. Jordan Hayes".to_string())
        .await?;
    println!("Response: {:?}\n", result.response);

    match result.context_data {
        shinkai_graphrag::search::base::ContextData::Dictionary(dict) => {
            for (entity, df) in dict.iter() {
                println!("Data: {} ({})", entity, df.height());
                println!("{:?}", df.head(Some(10)));
            }
        }
        data => {
            println!("Context data: {:?}", data);
        }
    }

    Ok(())
}
