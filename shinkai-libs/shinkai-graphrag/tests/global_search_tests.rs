use polars::{io::SerReader, prelude::ParquetReader};
use shinkai_graphrag::{
    context_builder::{
        community_context::GlobalCommunityContext, context_builder::ContextBuilderParams,
        indexer_entities::read_indexer_entities, indexer_reports::read_indexer_reports,
    },
    llm::llm::LLMParams,
    search::global_search::global_search::{GlobalSearch, GlobalSearchParams},
};
use tiktoken_rs::tokenizer::Tokenizer;
use utils::openai::ChatOpenAI;

mod utils;

#[tokio::test]
async fn global_search_test() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GRAPHRAG_API_KEY").unwrap();
    let llm_model = std::env::var("GRAPHRAG_LLM_MODEL").unwrap();

    let llm = ChatOpenAI::new(Some(api_key), llm_model, 5);
    let token_encoder = Tokenizer::Cl100kBase;

    // Load community reports
    // Download dataset: https://microsoft.github.io/graphrag/data/operation_dulce/dataset.zip

    let input_dir = "./dataset";
    let community_report_table = "create_final_community_reports";
    let entity_table = "create_final_nodes";
    let entity_embedding_table = "create_final_entities";

    let community_level = 2;

    let mut entity_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_table)).unwrap();
    let entity_df = ParquetReader::new(&mut entity_file).finish().unwrap();

    let mut report_file = std::fs::File::open(format!("{}/{}.parquet", input_dir, community_report_table)).unwrap();
    let report_df = ParquetReader::new(&mut report_file).finish().unwrap();

    let mut entity_embedding_file =
        std::fs::File::open(format!("{}/{}.parquet", input_dir, entity_embedding_table)).unwrap();
    let entity_embedding_df = ParquetReader::new(&mut entity_embedding_file).finish().unwrap();

    let reports = read_indexer_reports(&report_df, &entity_df, community_level)?;
    let entities = read_indexer_entities(&entity_df, &entity_embedding_df, community_level)?;

    println!("Reports: {:?}", report_df.head(Some(5)));

    // Build global context based on community reports

    let context_builder = GlobalCommunityContext::new(reports, Some(entities), Some(token_encoder));

    let context_builder_params = ContextBuilderParams {
        use_community_summary: false, // False means using full community reports. True means using community short summaries.
        shuffle_data: true,
        include_community_rank: true,
        min_community_rank: 0,
        community_rank_name: String::from("rank"),
        include_community_weight: true,
        community_weight_name: String::from("occurrence weight"),
        normalize_community_weight: true,
        max_tokens: 12_000, // change this based on the token limit you have on your model (if you are using a model with 8k limit, a good setting could be 5000)
        context_name: String::from("Reports"),
        column_delimiter: String::from("|"),
    };

    let map_llm_params = LLMParams {
        max_tokens: 1000,
        temperature: 0.0,
        response_format: std::collections::HashMap::from([("type".to_string(), "json_object".to_string())]),
    };

    let reduce_llm_params = LLMParams {
        max_tokens: 2000,
        temperature: 0.0,
        response_format: std::collections::HashMap::new(),
    };

    // Perform global search

    let search_engine = GlobalSearch::new(GlobalSearchParams {
        llm: Box::new(llm),
        context_builder,
        token_encoder: Some(token_encoder),
        map_system_prompt: None,
        reduce_system_prompt: None,
        response_type: String::from("multiple paragraphs"),
        allow_general_knowledge: false,
        general_knowledge_inclusion_prompt: None,
        json_mode: true,
        callbacks: None,
        max_data_tokens: 12_000,
        map_llm_params,
        reduce_llm_params,
        context_builder_params,
    });

    let result = search_engine
        .asearch(
            "What is the major conflict in this story and who are the protagonist and antagonist?".to_string(),
            None,
        )
        .await?;

    println!("Response: {:?}", result.response);

    println!("Context: {:?}", result.context_data);

    println!("LLM calls: {}. LLM tokens: {}", result.llm_calls, result.prompt_tokens);

    Ok(())
}
