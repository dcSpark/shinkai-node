use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::source::VRSource;
use shinkai_vector_resources::vector_resource::{
    FilterMode, NodeContent, ScoringMode, TraversalMethod, TraversalOption, VectorResource,
};
use shinkai_vector_resources::vector_resource_types::VRPath;
use std::collections::HashMap;

#[test]
fn test_remote_embeddings_generation() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let dog_embeddings = generator.generate_embedding_default_blocking("dog").unwrap();
    let cat_embeddings = generator.generate_embedding_default_blocking("cat").unwrap();

    assert_eq!(dog_embeddings, dog_embeddings);
    assert_eq!(cat_embeddings, cat_embeddings);
    assert_ne!(dog_embeddings, cat_embeddings);
}

#[tokio::test]
async fn test_remote_embeddings_generation_async_batched() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let inputs = vec![
        "dog", "cat", "lion", "tiger", "elephant", "giraffe", "zebra", "bear", "wolf", "fox",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<_>>();
    let ids = vec!["".to_string(); inputs.len()];
    let embeddings = generator.generate_embeddings(&inputs, &ids).await.unwrap();

    for (animal, embedding) in inputs.iter().zip(embeddings.iter()) {
        println!("Embedding for {}: {:?}", animal, embedding);
    }

    assert_ne!(embeddings[0], embeddings[1]);
    assert_ne!(embeddings[0], embeddings[2]);
    assert_ne!(embeddings[0], embeddings[3]);
    assert_ne!(embeddings[0], embeddings[4]);
    assert_ne!(embeddings[0], embeddings[5]);
    assert_ne!(embeddings[0], embeddings[6]);
    assert_ne!(embeddings[0], embeddings[7]);
    assert_ne!(embeddings[0], embeddings[8]);
    assert_ne!(embeddings[0], embeddings[9]);
}

#[test]
fn test_manual_resource_vector_search() {
    let generator = RemoteEmbeddingGenerator::new_default();

    //
    // Create a first resource
    //
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSource::new_uri_ref("animalwildlife.com"),
        "animal_resource",
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(&generator, vec!["animal".to_string(), "wild life".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embeddings = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embeddings = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embeddings = generator.generate_embedding_default_blocking(fact3).unwrap();
    doc.append_text_node(fact1, None, &fact1_embeddings, &vec![]);
    doc.append_text_node(fact2, None, &fact2_embeddings, &vec![]);
    doc.append_text_node(fact3, None, &fact3_embeddings, &vec![]);

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing basic vector search works
    let query_string = "What animal barks?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = doc.vector_search(query_embedding1.clone(), 1);
    assert_eq!(fact1, res[0].node.get_text_content().unwrap());

    let query_string2 = "What animal is slow?";
    let query_embedding2 = generator.generate_embedding_default_blocking(query_string2).unwrap();
    let res2 = doc.vector_search(query_embedding2, 3);
    assert_eq!(fact2, res2[0].node.get_text_content().unwrap());

    let query_string3 = "What animal swims in the ocean?";
    let query_embedding3 = generator.generate_embedding_default_blocking(query_string3).unwrap();
    let res3 = doc.vector_search(query_embedding3, 2);
    assert_eq!(fact3, res3[0].node.get_text_content().unwrap());

    //
    // Create a 2nd resource, a MapVectorResource
    //
    let mut map_resource = MapVectorResource::new_empty(
        "Tech Facts",
        Some("A collection of facts about technology"),
        VRSource::new_uri_ref("veryrealtechfacts.com"),
        "tech_resource",
    );

    map_resource.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    map_resource
        .update_resource_embedding_blocking(&generator, vec!["technology".to_string(), "phones".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the map resource
    let fact4 = "Phones provide the power of the internet in your pocket.";
    let fact4_embeddings = generator.generate_embedding_default_blocking(fact4).unwrap();
    map_resource.insert_text_node("some_key", fact4, None, &fact4_embeddings, &vec![]);

    // Insert the document resource into the map resource
    // To allow for this composability we need to convert the doc into a BaseVectorResource
    let doc_resource = BaseVectorResource::from(doc);
    map_resource.insert_vector_resource_node("doc_key", doc_resource, None);

    //
    // Create a third resource, a DocumentVectorResource about fruits
    //
    let mut fruit_doc = DocumentVectorResource::new_empty(
        "Fruit Facts",
        Some("A collection of facts about fruits"),
        VRSource::new_uri_ref("ostensiblyrealfruitfacts.com"),
        "fruit_resource",
    );
    fruit_doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice

    // Prepare embeddings + data, then add it to the fruit doc
    let fact5 = "Apples are sweet and crunchy.";
    let fact5_embeddings = generator.generate_embedding_default_blocking(fact5).unwrap();
    let fact6 = "Bananas are tasty and come in their own natural packaging.";
    let fact6_embeddings = generator.generate_embedding_default_blocking(fact6).unwrap();
    fruit_doc.append_text_node(fact5, None, &fact5_embeddings, &vec![]);
    fruit_doc.append_text_node(fact6, None, &fact6_embeddings, &vec![]);

    // Insert the map resource into the fruit doc
    let map_resource = BaseVectorResource::from(map_resource);
    fruit_doc.append_vector_resource_node(map_resource, None);

    //
    // Perform Vector Search Tests Through All Levels/Resources
    //

    // Perform a vector search for data 2 levels lower in the fruit doc to ensure
    // that vector searches propagate inwards through all resources
    let res = fruit_doc.vector_search(query_embedding1.clone(), 5);
    assert_eq!(fact1, res[0].node.get_text_content().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform a vector search for data 1 level lower in the tech map resource
    let query_string = "What can I use to access the internet?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact4, res[0].node.get_text_content().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/some_key", res[0].format_path_to_string());
    assert_eq!(1, res[0].retrieval_path.depth());

    // Perform a vector search on the fruit doc
    // for data on the base level
    let query_string = "What fruit has its own packaging?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding.clone(), 10);
    assert_eq!(fact6, res[0].node.get_text_content().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/2", res[0].format_path_to_string());
    assert_eq!(0, res[0].retrieval_path.depth());

    //
    // Traversal Tests
    //
    // Perform UntilDepth(0) traversal to ensure it is working properly, assert the dog fact1 cant be found
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Efficient,
        &vec![TraversalOption::UntilDepth(0)],
        None,
    );
    assert_ne!(fact1, res[0].node.get_text_content().unwrap());
    assert_eq!(0, res[0].retrieval_path.depth());
    // Perform UntilDepth(1) traversal to ensure it is working properly, assert the BaseVectorResource for animals is found (not fact1)
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::UntilDepth(1)],
        None,
    );
    assert_eq!(
        "3 Animal Facts",
        res[0]
            .node
            .get_vector_resource_content()
            .unwrap()
            .as_trait_object()
            .name()
    );
    // Perform UntilDepth(2) traversal to ensure it is working properly, assert dog fact1 is found at the correct depth
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::UntilDepth(2)],
        None,
    );
    assert_eq!(NodeContent::Text(fact1.to_string()), res[0].node.content);
    // Perform MinimumScore option with impossible score to ensure it is working properly
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::MinimumScore(0.99)],
        None,
    );
    assert_eq!(res.len(), 0);

    // Perform MinimumScore option with low score to ensure it is working properly
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::MinimumScore(0.01)],
        None,
    );
    assert!(res.len() > 0);

    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform Exhaustive traversal to ensure it is working properly, assert dog fact1 is found at the correct depth
    // By requesting only 1 result, Efficient traversal does not go deeper, while Exhaustive makes it all the way to the bottom
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        1,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        None,
    );
    assert_eq!(NodeContent::Text(fact1.to_string()), res[0].node.content);
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        1,
        TraversalMethod::Efficient,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        None,
    );
    assert_ne!(NodeContent::Text(fact1.to_string()), res[0].node.content);

    //
    // Path Tests
    //
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        None,
    );
    assert_eq!(res.len(), 6);
    let path = VRPath::from_path_string("/3/");
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        Some(path),
    );
    assert_eq!(res.len(), 4);
    let path = VRPath::from_path_string("/3/doc_key/");
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        Some(path),
    );
    assert_eq!(res.len(), 3);

    //
    /// Metadata Filter Tests
    //
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetFilterMode(
            FilterMode::ContainsAnyMetadataKeyValues(vec![
                ("key".to_string(), Some("value".to_string())),
                ("other_key".to_string(), None),
            ]),
        )],
        None,
    );
    assert_eq!(res.len(), 0);

    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetFilterMode(
            FilterMode::ContainsAllMetadataKeyValues(vec![
                ("key".to_string(), Some("value".to_string())),
                ("other_key".to_string(), None),
            ]),
        )],
        None,
    );
    assert_eq!(res.len(), 0);

    // Creating fake metadata to test with
    let mut hm1 = HashMap::new();
    hm1.insert("common_key".to_string(), "common_value".to_string());
    hm1.insert("unique_key1".to_string(), "unique_value1".to_string());

    let mut hm2 = HashMap::new();
    hm2.insert("common_key".to_string(), "common_value".to_string());
    hm2.insert("unique_key2".to_string(), "unique_value2".to_string());

    fruit_doc.append_text_node(fact5, Some(hm1), &fact5_embeddings, &vec![]);
    fruit_doc.append_text_node(fact6, Some(hm2), &fact6_embeddings, &vec![]);

    // Check any filtering, with the common key/value
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetFilterMode(
            FilterMode::ContainsAnyMetadataKeyValues(vec![
                ("uniq".to_string(), Some("e".to_string())),
                ("common_key".to_string(), Some("common_value".to_string())),
            ]),
        )],
        None,
    );
    assert_eq!(res.len(), 2);

    // Check all filtering, including with None value skipping
    let res = fruit_doc.vector_seach_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetFilterMode(
            FilterMode::ContainsAllMetadataKeyValues(vec![
                ("common_key".to_string(), None),
                ("unique_key2".to_string(), Some("unique_value2".to_string())),
            ]),
        )],
        None,
    );
    assert_eq!(res.len(), 1);

    // Check the metadata_index
    println!("Metdata index: {:?}", fruit_doc.metadata_index());
    assert_eq!(fruit_doc.metadata_index().get_all_metadata_keys().len(), 3);
}

#[test]
fn test_manual_syntactic_vector_search() {
    let generator = RemoteEmbeddingGenerator::new_default();

    //
    // Create a first resource
    //
    let mut doc = DocumentVectorResource::new_empty(
        "CV Data From Resume",
        Some("A bunch of data theoretically parsed out of a CV"),
        VRSource::None,
        "cv_data",
    );
    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(&generator, vec!["cv".to_string(), "email".to_string()])
        .unwrap();

    // Manually create a few test tags
    let regex1 = r#"[€$¥£][0-9]{1,3}(,[0-9]{3})*(\.[0-9]{2})?\b|\b€[0-9]{1,3}(\.[0-9]{3})*,(0-9{2})?"#;
    let price_tag = DataTag::new("Price", "A price in a major currency", regex1).unwrap();

    let regex2 = r#"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"#;
    let email_tag = DataTag::new("Email", "An email address", regex2).unwrap();

    let regex3 = r#"(19|20)\d\d[- /.](0[1-9]|1[012])[- /.](0[1-9]|[12][0-9]|3[01])|(0[1-9]|1[012])[- /.](0[1-9]|[12][0-9]|3[01])[- /.](19|20)\d\d|(0[1-9]|[12][0-9]|3[01])[- /.](0[1-9]|1[012])[- /.](19|20)\d\d"#;
    let date_tag = DataTag::new(
        "Date",
        "Captures dates in three common formats - YYYY-MM-DD, MM/DD/YYYY, and DD/MM/YYYY.",
        regex3,
    )
    .unwrap();

    let regex4 = r#"[0-9]+x"#;
    let multiplier_tag = DataTag::new("Multiplier", "Strings like `100x` which denote a multiplier.", regex4).unwrap();

    let data_tags = vec![
        price_tag.clone(),
        email_tag.clone(),
        date_tag.clone(),
        multiplier_tag.clone(),
    ];

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Name: Joe Smith - Email: joesmith@gmail.com";
    let fact1_embeddings = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Birthday: 23/03/1980";
    let fact2_embeddings = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Previous Accomplishments: Drove $1,500,000 in sales at my previous company, which translate to a 4x improvement compared to when I joined.";
    let fact3_embeddings = generator.generate_embedding_default_blocking(fact3).unwrap();
    doc.append_text_node(fact1, None, &fact1_embeddings, &data_tags);
    doc.append_text_node(fact2, None, &fact2_embeddings, &data_tags);
    doc.append_text_node(fact3, None, &fact3_embeddings, &data_tags);

    // println!("Doc data tag index: {:?}", doc.data_tag_index());

    // Email syntactic vector search
    // In Shinkai the LLM Agent would do a Tag Vector Search in node DB to find the email_tag based on user's prompt
    // And then calls syntactic_vector_search to guarantee the data retrieved is of the correct structure/"type"
    let query = generator
        .generate_embedding_default_blocking("What is the applicant's email?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 1, &vec![email_tag.name.clone()]);
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!(NodeContent::Text(fact1.to_string()), fetched_node.node.content);

    // Date syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("What is the applicant's birthday?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 10, &vec![date_tag.name.clone()]);
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!(NodeContent::Text(fact2.to_string()), fetched_node.node.content);

    // Price syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 2, &vec![price_tag.name.clone()]);
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!(NodeContent::Text(fact3.to_string()), fetched_node.node.content);

    // Multiplier syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 5, &vec![multiplier_tag.name.clone()]);
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!(NodeContent::Text(fact3.to_string()), fetched_node.node.content);
}
