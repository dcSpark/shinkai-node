use shinkai_vector_resources::{
    embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
    file_parser::{
        file_parser::{FileParser, ShinkaiFileParser},
        unstructured_api::UnstructuredAPI,
    },
    source::DistributionInfo,
};

#[tokio::test]
async fn local_pdf_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "shinkai_intro.pdf";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParserType::Local,
    )
    .await
    .unwrap();

    resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, false, false);

    // Perform vector search
    let query_string = "What is Shinkai?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.7);
}
