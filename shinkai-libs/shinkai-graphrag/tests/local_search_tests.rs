use polars::{io::SerReader, prelude::ParquetReader};
use shinkai_graphrag::{
    indexer_adapters::{
        read_indexer_entities, read_indexer_relationships, read_indexer_reports, read_indexer_text_units,
    },
    input::loaders::dfs::store_entity_semantic_embeddings,
    llm::base::LLMParams,
    search::local_search::{
        mixed_context::{LocalSearchMixedContext, MixedContextBuilderParams},
        search::LocalSearch,
    },
    vector_stores::lancedb::LanceDBVectorStore,
};
use utils::openai::{num_tokens, ChatOpenAI, OpenAIEmbedding};

mod utils;

#[tokio::test]
async fn openai_local_search_test() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GRAPHRAG_API_KEY").unwrap();
    let llm_model = std::env::var("GRAPHRAG_LLM_MODEL").unwrap();
    let embedding_model = std::env::var("GRAPHRAG_EMBEDDING_MODEL").unwrap();

    let llm = ChatOpenAI::new(Some(api_key.clone()), llm_model, 5);
    let text_embedder = OpenAIEmbedding::new(Some(api_key), embedding_model, 8191, 5);

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
    let local_context_params = MixedContextBuilderParams {
        text_unit_prop: 0.5,
        community_prop: 0.1,
        top_k_mapped_entities: 10,
        top_k_relationships: 10,
        include_entity_rank: true,
        include_relationship_weight: true,
        include_community_rank: false,
        return_candidate_context: false,
        max_tokens: 12_000,

        query: "".to_string(),
        include_entity_names: None,
        exclude_entity_names: None,
        rank_description: "number of relationships".to_string(),
        relationship_ranking_attribute: "rank".to_string(),
        use_community_summary: false,
        min_community_rank: 0,
        community_context_name: "Reports".to_string(),
        column_delimiter: "|".to_string(),
    };

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
    println!("Response: {:?}", result.response);

    let result = search_engine
        .asearch("Tell me about Dr. Jordan Hayes".to_string())
        .await?;
    println!("Response: {:?}", result.response);

    println!("Context: {:?}", result.context_data);

    Ok(())
}
