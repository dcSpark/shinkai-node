use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::file_parser::file_parser::{FileParser, ShinkaiFileParser};
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::source::{DistributionInfo, VRSourceReference};
use shinkai_vector_resources::vector_resource::document_resource::DocumentVectorResource;
use shinkai_vector_resources::vector_resource::map_resource::MapVectorResource;
use shinkai_vector_resources::vector_resource::vrkai::VRKai;
use shinkai_vector_resources::vector_resource::vrpack::VRPack;
use shinkai_vector_resources::vector_resource::BaseVectorResource;
use shinkai_vector_resources::vector_resource::{
    FilterMode, NodeContent, ResultsMode, ScoringMode, TraversalMethod, TraversalOption, VectorResourceCore,
    VectorResourceSearch,
};
use shinkai_vector_resources::vector_resource::{RetrievedNode, VRPath};
use std::collections::HashMap;

pub fn default_vector_resource_doc() -> DocumentVectorResource {
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSourceReference::new_uri_ref("animalwildlife.com"),
        true,
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(&generator, Some(vec!["animal".to_string(), "wild life".to_string()]))
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embedding = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embedding = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embedding = generator.generate_embedding_default_blocking(fact3).unwrap();
    let _ = doc
        .append_text_node(fact1, None, fact1_embedding.clone(), &vec![])
        .unwrap();
    let _ = doc
        .append_text_node(fact2, None, fact2_embedding.clone(), &vec![])
        .unwrap();
    let _ = doc
        .append_text_node(fact3, None, fact3_embedding.clone(), &vec![])
        .unwrap();
    return doc;
}

fn default_vr_kai() -> VRKai {
    let resource = BaseVectorResource::Document(default_vector_resource_doc());
    VRKai::new(resource, None)
}

fn default_vr_pack() -> VRPack {
    let vrkai = default_vr_kai();
    let mut vrpack = VRPack::new_empty("");
    let _ = vrpack.insert_vrkai(&vrkai, VRPath::root(), true);
    vrpack
}

#[test]
fn test_vr_kai_prepare_and_parse_methods() {
    let vr_kai = default_vr_kai();

    // Test encode_as_base64 and from_base64
    let base64_encoded = vr_kai.encode_as_base64().expect("Failed to prepare as base64");
    let parsed_from_base64 = VRKai::from_base64(&base64_encoded).expect("Failed to parse from base64");
    assert_eq!(
        serde_json::to_string(&vr_kai).unwrap(),
        serde_json::to_string(&parsed_from_base64).unwrap()
    );

    // Test encode_as_bytes and from_bytes
    let bytes_encoded = vr_kai.encode_as_bytes().expect("Failed to prepare as bytes");
    let parsed_from_bytes = VRKai::from_bytes(&bytes_encoded).expect("Failed to parse from bytes");
    assert_eq!(
        serde_json::to_string(&vr_kai).unwrap(),
        serde_json::to_string(&parsed_from_bytes).unwrap()
    );

    // Test to_json and from_json for completeness
    let json_str = vr_kai.to_json().expect("Failed to convert to JSON");
    let parsed_from_json = VRKai::from_json(&json_str).expect("Failed to parse from JSON");
    assert_eq!(
        serde_json::to_string(&vr_kai).unwrap(),
        serde_json::to_string(&parsed_from_json).unwrap()
    );
}

#[test]
fn test_vr_pack_prepare_and_parse_methods() {
    let vr_pack = default_vr_pack();

    // Test encode_as_base64 and from_base64
    let base64_encoded = vr_pack.encode_as_base64().expect("Failed to prepare as base64");
    let parsed_from_base64 = VRPack::from_base64(&base64_encoded).expect("Failed to parse from base64");
    assert_eq!(
        serde_json::to_string(&vr_pack).unwrap(),
        serde_json::to_string(&parsed_from_base64).unwrap()
    );

    // Test encode_as_bytes and from_bytes
    let bytes_encoded = vr_pack.encode_as_bytes().expect("Failed to prepare as bytes");
    let parsed_from_bytes = VRPack::from_bytes(&bytes_encoded).expect("Failed to parse from bytes");
    assert_eq!(
        serde_json::to_string(&vr_pack).unwrap(),
        serde_json::to_string(&parsed_from_bytes).unwrap()
    );

    // Test to_json and from_json for completeness
    let json_str = vr_pack.to_json().expect("Failed to convert to JSON");
    let parsed_from_json = VRPack::from_json(&json_str).expect("Failed to parse from JSON");
    assert_eq!(
        serde_json::to_string(&vr_pack).unwrap(),
        serde_json::to_string(&parsed_from_json).unwrap()
    );
}

#[test]
fn test_remote_embedding_generation() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let dog_embedding = generator.generate_embedding_default_blocking("dog").unwrap();
    let cat_embedding = generator.generate_embedding_default_blocking("cat").unwrap();

    assert_eq!(dog_embedding, dog_embedding);
    assert_eq!(cat_embedding, cat_embedding);
    assert_ne!(dog_embedding, cat_embedding);
}

#[tokio::test]
async fn test_remote_embedding_generation_async_batched() {
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
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let _fact1_embedding = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let _fact2_embedding = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let _fact3_embedding = generator.generate_embedding_default_blocking(fact3).unwrap();

    let doc = default_vector_resource_doc();

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing basic vector search works
    let query_string = "What animal barks?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = doc.vector_search(query_embedding1.clone(), 1);
    assert_eq!(fact1, res[0].node.get_text_content().unwrap().to_string());

    let query_string2 = "What animal is slow?";
    let query_embedding2 = generator.generate_embedding_default_blocking(query_string2).unwrap();
    let res2 = doc.vector_search(query_embedding2.clone(), 3);
    assert_eq!(fact2, res2[0].node.get_text_content().unwrap().to_string());

    let query_string3 = "What animal swims in the ocean?";
    let query_embedding3 = generator.generate_embedding_default_blocking(query_string3).unwrap();
    let res3 = doc.vector_search(query_embedding3, 2);
    assert_eq!(fact3, res3[0].node.get_text_content().unwrap().to_string());

    //
    // Create a 2nd resource, a MapVectorResource
    //
    let mut map_resource = MapVectorResource::new_empty(
        "Tech Facts",
        Some("A collection of facts about technology"),
        VRSourceReference::new_uri_ref("veryrealtechfacts.com"),
        true,
    );

    map_resource.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    map_resource
        .update_resource_embedding_blocking(&generator, Some(vec!["technology".to_string(), "phones".to_string()]))
        .unwrap();

    // Prepare embeddings + data, then add it to the map resource
    let fact4 = "Phones provide the power of the internet in your pocket.";
    let fact4_embedding = generator.generate_embedding_default_blocking(fact4).unwrap();
    let _ = map_resource.insert_text_node(
        "some_key".to_string(),
        fact4.to_string(),
        None,
        fact4_embedding.clone(),
        &vec![],
    );

    // Insert the document resource into the map resource
    // To allow for this composability we need to convert the doc into a BaseVectorResource
    let doc_resource = BaseVectorResource::from(doc);
    let _ = map_resource.insert_vector_resource_node_auto("doc_key", doc_resource, None);

    //
    // Create a third resource, a DocumentVectorResource about fruits
    //
    let mut fruit_doc = DocumentVectorResource::new_empty(
        "Fruit Facts",
        Some("A collection of facts about fruits"),
        VRSourceReference::new_uri_ref("ostensiblyrealfruitfacts.com"),
        true,
    );
    fruit_doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice

    // Prepare embeddings + data, then add it to the fruit doc
    let fact5 = "Apples are sweet and crunchy.";
    let fact5_embedding = generator.generate_embedding_default_blocking(fact5).unwrap();
    let fact6 = "Bananas are tasty and come in their own natural packaging.";
    let fact6_embedding = generator.generate_embedding_default_blocking(fact6).unwrap();
    let _ = fruit_doc.append_text_node(fact5, None, fact5_embedding.clone(), &vec![]);
    let _ = fruit_doc.append_text_node(fact6, None, fact6_embedding.clone(), &vec![]);

    // Insert the map resource into the fruit doc
    let map_resource = BaseVectorResource::from(map_resource);
    let mut new_map_resource = map_resource.as_map_resource_cloned().unwrap();
    let _ = fruit_doc.append_vector_resource_node_auto(map_resource, None);

    //
    // Perform Vector Search Tests Through All Levels/Resources
    //

    // Perform a vector search for data 2 levels lower in the fruit doc to ensure
    // that vector searches propagate inwards through all resources
    let res = fruit_doc.vector_search(query_embedding1.clone(), 5);
    assert_eq!(fact1, res[0].node.get_text_content().unwrap().to_string());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform a vector search for data 1 level lower in the tech map resource
    let query_string = "What can I use to access the internet?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact4, res[0].node.get_text_content().unwrap().to_string());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/some_key", res[0].format_path_to_string());
    assert_eq!(1, res[0].retrieval_path.depth());

    // Perform a vector search on the fruit doc
    // for data on the base level
    let query_string = "What fruit has its own packaging?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding.clone(), 10);
    assert_eq!(fact6, res[0].node.get_text_content().unwrap().to_string());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/2", res[0].format_path_to_string());
    assert_eq!(0, res[0].retrieval_path.depth());

    //
    // Traversal Tests
    //
    // Perform UntilDepth(0) traversal to ensure it is working properly, assert the dog fact1 cant be found
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Efficient,
        &vec![TraversalOption::UntilDepth(0)],
        None,
    );
    assert_ne!(fact1, res[0].node.get_text_content().unwrap().to_string());
    assert_eq!(0, res[0].retrieval_path.depth());
    // Perform UntilDepth(1) traversal to ensure it is working properly, assert the BaseVectorResource for animals is found (not fact1)
    let res = fruit_doc.vector_search_customized(
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
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::UntilDepth(2)],
        None,
    );
    assert_eq!(NodeContent::Text(fact1.to_string()), res[0].node.content);
    // Perform MinimumScore option with impossible score to ensure it is working properly
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::MinimumScore(0.99)],
        None,
    );
    assert_eq!(res.len(), 0);

    // Perform MinimumScore option with low score to ensure it is working properly
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        5,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::MinimumScore(0.01)],
        None,
    );
    assert!(!res.is_empty());

    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform Exhaustive traversal to ensure it is working properly, assert dog fact1 is found at the correct depth
    // By requesting only 1 result, Efficient traversal does not go deeper, while Exhaustive makes it all the way to the bottom
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        1,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        None,
    );
    assert_eq!(NodeContent::Text(fact1.to_string()), res[0].node.content);
    let res = fruit_doc.vector_search_customized(
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
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        None,
    );
    assert_eq!(res.len(), 6);
    let path = VRPath::from_string("/3/").unwrap();
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        Some(path),
    );
    assert_eq!(res.len(), 4);
    let path = VRPath::from_string("/3/doc_key/").unwrap();
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        Some(path),
    );
    assert_eq!(res.len(), 3);

    // Metadata Filter Tests
    let res = fruit_doc.vector_search_customized(
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

    let res = fruit_doc.vector_search_customized(
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

    let _ = fruit_doc.append_text_node(fact5, Some(hm1), fact5_embedding.clone(), &vec![]);
    let _ = fruit_doc.append_text_node(fact6, Some(hm2), fact6_embedding.clone(), &vec![]);

    // Check any filtering, with the common key/value
    let res = fruit_doc.vector_search_customized(
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
    let res = fruit_doc.vector_search_customized(
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

    // Check Proximity search results mode
    let res = fruit_doc.vector_search_customized(
        query_embedding1.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(1, 1))],
        None,
    );
    new_map_resource.print_all_nodes_exhaustive(None, true, false);
    assert_eq!(res.len(), 2);
    let res = fruit_doc.vector_search_customized(
        query_embedding2.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(1, 1))],
        None,
    );
    new_map_resource.print_all_nodes_exhaustive(None, true, false);
    assert_eq!(res.len(), 3);

    // The nodes are already included in the first top results proximity, so this checks that there's no more.
    let res = fruit_doc.vector_search_customized(
        query_embedding2.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(2, 1))],
        None,
    );
    assert_eq!(res.len(), 3);

    let _ = fruit_doc.append_text_node(fact6, None, fact6_embedding.clone(), &vec![]);
    let _ = fruit_doc.append_text_node(fact6, None, fact6_embedding.clone(), &vec![]);

    println!("\n\nFruit doc:");
    fruit_doc.print_all_nodes_exhaustive(None, true, false);

    // Check that proximity window works
    let query_string = "Whats an apple?";
    let query_embedding_fruit = generator.generate_embedding_default_blocking(query_string).unwrap();

    let res = fruit_doc.vector_search_customized(
        query_embedding_fruit.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(1, 2))],
        None,
    );
    assert_eq!(res.len(), 5);

    let res = fruit_doc.vector_search_customized(
        query_embedding_fruit.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(2, 2))],
        None,
    );

    assert_eq!(res.len(), 6);

    // Verify proximity grouping is working
    let res = fruit_doc.vector_search_customized(
        query_embedding_fruit.clone(),
        100,
        TraversalMethod::Exhaustive,
        &vec![TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(1, 2))],
        None,
    );

    let grouped_results = RetrievedNode::group_proximity_results(&res).unwrap();

    for (index, group) in grouped_results.iter().enumerate() {
        println!("Group {}:", index);
        for result in group {
            println!("Result: {:?}", result.retrieval_path);
        }
    }
    assert_eq!(grouped_results.len(), 2);
    assert_eq!(grouped_results[0].len(), 3);
    assert_eq!(grouped_results[1].len(), 2);

    // Check the metadata_index
    println!("Metdata index: {:?}", fruit_doc.metadata_index());
    assert_eq!(fruit_doc.metadata_index().get_all_metadata_keys().len(), 3);

    // At path method tests

    // Insert/retrieve tests
    let path = VRPath::from_string("/doc_key/").unwrap();
    new_map_resource
        .insert_vector_resource_node_at_path(
            path,
            "4",
            BaseVectorResource::Map(new_map_resource.clone()),
            None,
            new_map_resource.resource_embedding().clone(),
        )
        .unwrap();
    let test_path = VRPath::from_string("/doc_key/4/doc_key/3").unwrap();
    let res = new_map_resource.retrieve_node_at_path(test_path.clone(), None).unwrap();
    assert_eq!(res.node.id, "3");
    assert_eq!(res.retrieval_path.to_string(), test_path.to_string());

    // Validate embedding retrieval works by regenerating the embedding from the text
    let embedding = new_map_resource.retrieve_embedding_at_path(test_path.clone()).unwrap();
    match res.node.content {
        NodeContent::Text(text) => {
            let regenerated_embedding = generator.generate_embedding_blocking(&text, "3").unwrap();
            assert_eq!(embedding, regenerated_embedding);
        }
        _ => panic!("Node content is not text"),
    }
    // Proximity retrieval test
    let test_path = VRPath::from_string("/doc_key/4/doc_key/3").unwrap();
    new_map_resource.print_all_nodes_exhaustive(None, true, false);
    let res = new_map_resource
        .proximity_retrieve_nodes_at_path(test_path.clone(), 1, None)
        .unwrap();
    assert_eq!(res.len(), 2);
    let test_path = VRPath::from_string("/doc_key/4/doc_key/2").unwrap();
    let res = new_map_resource
        .proximity_retrieve_nodes_at_path(test_path.clone(), 1, None)
        .unwrap();
    assert_eq!(res.len(), 3);
    let test_path = VRPath::from_string("/doc_key/4/doc_key/1").unwrap();
    let res = new_map_resource
        .proximity_retrieve_nodes_at_path(test_path.clone(), 1, None)
        .unwrap();
    assert_eq!(res.len(), 2);
    let res = new_map_resource
        .proximity_retrieve_nodes_at_path(test_path.clone(), 5000, None)
        .unwrap();
    assert_eq!(res.len(), 3);

    // Check that no node is retrieved after removing it by path
    let test_path = VRPath::from_string("/doc_key/4/doc_key/3").unwrap();
    let _ = new_map_resource.remove_node_at_path(test_path.clone(), true);
    let res = new_map_resource.retrieve_node_at_path(test_path.clone(), None);
    assert!(!res.is_ok());

    // Replace an existing node in a Map Resource and validate it's been changed
    let test_path = VRPath::from_string("/doc_key/4/some_key").unwrap();
    let initial_node = new_map_resource.retrieve_node_at_path(test_path.clone(), None).unwrap();
    new_map_resource
        .replace_with_text_node_at_path(
            test_path.clone(),
            "----My new node value----".to_string(),
            None,
            fact6_embedding.clone(),
            vec![],
        )
        .unwrap();
    let new_node = new_map_resource.retrieve_node_at_path(test_path.clone(), None).unwrap();
    assert_ne!(initial_node, new_node);
    assert_eq!(
        NodeContent::Text("----My new node value----".to_string()),
        new_node.node.content
    );

    // Replace an existing node in a Doc Resource and validate it's been changed
    let test_path = VRPath::from_string("/doc_key/4/doc_key/2").unwrap();
    let initial_node = new_map_resource.retrieve_node_at_path(test_path.clone(), None).unwrap();
    new_map_resource
        .replace_with_text_node_at_path(
            test_path.clone(),
            "----My new node value 2----".to_string(),
            None,
            fact6_embedding.clone(),
            vec![],
        )
        .unwrap();
    let new_node = new_map_resource.retrieve_node_at_path(test_path.clone(), None).unwrap();
    assert_ne!(initial_node, new_node);
    assert_eq!(
        NodeContent::Text("----My new node value 2----".to_string()),
        new_node.node.content
    );

    // Append a node into a Doc Resource and validate it's been added
    let mut fruit_doc = fruit_doc.clone();
    let path = VRPath::from_string("/3/doc_key/").unwrap();
    fruit_doc
        .append_text_node_at_path(
            path,
            "--- appended text node ---",
            None,
            new_map_resource.resource_embedding().clone(),
            &vec![],
        )
        .unwrap();
    let test_path = VRPath::from_string("/3/doc_key/4").unwrap();
    let res = fruit_doc.retrieve_node_at_path(test_path.clone(), None).unwrap();
    assert_eq!(res.node.id, "4");
    assert_eq!(res.retrieval_path.to_string(), test_path.to_string());

    // Pop the previously appended node
    let path = VRPath::from_string("/3/doc_key/").unwrap();
    fruit_doc.pop_node_at_path(path, true).unwrap();
    let test_path = VRPath::from_string("/3/doc_key/4").unwrap();
    let res = fruit_doc.retrieve_node_at_path(test_path.clone(), None);
    assert_eq!(res.is_ok(), false);

    //
    // Merkelization Tests
    //
    let path = VRPath::from_string("/3/doc_key/2").unwrap();
    let res = fruit_doc.retrieve_node_at_path(path.clone(), None).unwrap();
    let regened_merkle_hash = res.node._generate_merkle_hash().unwrap();
    assert_eq!(regened_merkle_hash, res.node.get_merkle_hash().unwrap());

    // Store the original Merkle hash
    let original_merkle_hash = fruit_doc.get_merkle_root().unwrap();

    // Append a node into a Doc Resource
    let path = VRPath::from_string("/3/doc_key/").unwrap();
    fruit_doc
        .append_text_node_at_path(
            path.clone(),
            "--- appended text node ---",
            None,
            new_map_resource.resource_embedding().clone(),
            &vec![],
        )
        .unwrap();

    // Retrieve and store the new Merkle hash
    let new_merkle_hash = fruit_doc.get_merkle_root().unwrap();
    assert_ne!(
        original_merkle_hash, new_merkle_hash,
        "Merkle hash should be different after append"
    );

    // Pop the previously appended node
    fruit_doc.pop_node_at_path(path, true).unwrap();

    // Retrieve the Merkle hash again and assert it's the same as the original
    let reverted_merkle_hash = fruit_doc.get_merkle_root().unwrap();
    assert_eq!(
        original_merkle_hash, reverted_merkle_hash,
        "Merkle hash should be the same as original after pop"
    );
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
        VRSourceReference::None,
        true,
    );
    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(&generator, Some(vec!["cv".to_string(), "email".to_string()]))
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
    let fact1_embedding = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Birthday: 23/03/1980";
    let fact2_embedding = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Previous Accomplishments: Drove $1,500,000 in sales at my previous company, which translate to a 4x improvement compared to when I joined.";
    let fact3_embedding = generator.generate_embedding_default_blocking(fact3).unwrap();
    let _ = doc.append_text_node(fact1, None, fact1_embedding.clone(), &data_tags);
    let _ = doc.append_text_node(fact2, None, fact2_embedding.clone(), &data_tags);
    let _ = doc.append_text_node(fact3, None, fact3_embedding.clone(), &data_tags);

    // println!("Doc data tag index: {:?}", doc.data_tag_index());

    // Email syntactic vector search
    // In Shinkai the LLM Agent would do a Tag Vector Search in node DB to find the email_tag based on user's prompt
    // And then calls syntactic_vector_search to guarantee the data retrieved is of the correct structure/"type"
    let query = generator
        .generate_embedding_default_blocking("What is the applicant's email?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 1, &[email_tag.name.clone()]);
    let fetched_node = fetched_data.first().unwrap();
    assert_eq!(NodeContent::Text(fact1.to_string()), fetched_node.node.content);

    // Date syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("What is the applicant's birthday?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 10, &[date_tag.name.clone()]);
    let fetched_node = fetched_data.first().unwrap();
    assert_eq!(NodeContent::Text(fact2.to_string()), fetched_node.node.content);

    // Price syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 2, &[price_tag.name.clone()]);
    let fetched_node = fetched_data.first().unwrap();
    assert_eq!(NodeContent::Text(fact3.to_string()), fetched_node.node.content);

    // Multiplier syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 5, &[multiplier_tag.name.clone()]);
    let fetched_node = fetched_data.first().unwrap();
    assert_eq!(NodeContent::Text(fact3.to_string()), fetched_node.node.content);
}

// #[test]
fn test_checking_embedding_similarity() {
    let generator = RemoteEmbeddingGenerator::new_default();

    //
    // Create a first resource
    //
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSourceReference::new_uri_ref("animalwildlife.com"),
        true,
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(&generator, Some(vec!["animal".to_string(), "wild life".to_string()]))
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embedding = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embedding = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embedding = generator.generate_embedding_default_blocking(fact3).unwrap();
    doc.append_text_node(fact1, None, fact1_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact2, None, fact2_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact3, None, fact3_embedding.clone(), &vec![])
        .unwrap();

    // Testing small alternations to the input text still retain a high similarity score
    let res = doc.vector_search(fact1_embedding.clone(), 1);
    assert_eq!(fact1, res[0].node.get_text_content().unwrap().to_string());
    assert!(res[0].score > 0.98);

    let fact1_embedding_2 = generator.generate_embedding_default_blocking(fact1).unwrap();
    let res = doc.vector_search(fact1_embedding_2.clone(), 1);
    assert!(res[0].score > 0.98);

    let similar_to_fact_1 = "Dogs are creatures with 4 legs that bark .";
    let similar_fact1_embedding = generator
        .generate_embedding_default_blocking(similar_to_fact_1)
        .unwrap();
    let res = doc.vector_search(similar_fact1_embedding.clone(), 1);
    println!("{} : {}", res[0].score, similar_to_fact_1);
    assert!(res[0].score > 0.98);

    let similar_to_fact_1 = "Dogs are creatures with 4 legs that bark";
    let similar_fact1_embedding = generator
        .generate_embedding_default_blocking(similar_to_fact_1)
        .unwrap();
    let res = doc.vector_search(similar_fact1_embedding.clone(), 1);
    println!("{} : {}", res[0].score, similar_to_fact_1);
    assert!(res[0].score > 0.98);

    let similar_to_fact_1 = "Dogs   are   creatures with 4   legs that   bark";
    let similar_fact1_embedding = generator
        .generate_embedding_default_blocking(similar_to_fact_1)
        .unwrap();
    let res = doc.vector_search(similar_fact1_embedding.clone(), 1);
    println!("{} : {}", res[0].score, similar_to_fact_1);
    assert!(res[0].score > 0.98);

    let similar_to_fact_1 = "Dogs --   are ||  creatures ~ with 4 legs, that   bark";
    let similar_fact1_embedding = generator
        .generate_embedding_default_blocking(similar_to_fact_1)
        .unwrap();
    let res = doc.vector_search(similar_fact1_embedding.clone(), 1);
    println!("{} : {}", res[0].score, similar_to_fact_1);
    assert!(res[0].score < 0.98);
}

#[tokio::test]
async fn test_embeddings_coherence() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSourceReference::new_uri_ref("animalwildlife.com"),
        true,
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding(&generator, Some(vec!["animal".to_string(), "wild life".to_string()]))
        .await
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embedding = generator.generate_embedding_default(fact1).await.unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embedding = generator.generate_embedding_default(fact2).await.unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embedding = generator.generate_embedding_default(fact3).await.unwrap();
    doc.append_text_node(fact1, None, fact1_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact2, None, fact2_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact3, None, fact3_embedding.clone(), &vec![])
        .unwrap();

    let cloned_doc = BaseVectorResource::Document(doc.clone());
    let _ = doc.append_vector_resource_node_auto(cloned_doc, None);

    assert!(doc.verify_internal_embeddings_coherence(&generator, 0.5).await.is_ok());
    assert!(doc.verify_internal_embeddings_coherence(&generator, 0.0).await.is_ok());
    assert!(doc.verify_internal_embeddings_coherence(&generator, 23.4).await.is_ok());
}

#[tokio::test]
async fn local_txt_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "canada.txt";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "Who's donnacona?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.3);
    assert!(results[0].node.get_text_content().unwrap().contains("Donnacona"));
    for result in results {
        // println!("{}:{}", result.score, result.node.get_text_content().unwrap());
        assert!(result.node.get_text_content().unwrap().len() > 200);
    }
}

#[tokio::test]
async fn local_csv_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "cars.csv";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "Which car has 495 horsepower?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.5);
    assert!(results[0].node.get_text_content().unwrap().contains("Corvette"));
}

#[tokio::test]
async fn local_malformed_csv_parsing_test() {
    let malformed_csv = "\
        Year,Make,Model,Description,Price
        1997,Ford,E350,\"ac, abs, moon\",3000
        1999,Chevy,\"Venture \"\"Extended Edition\"\"\",\"\"";

    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "cars.csv";
    let buffer = malformed_csv.as_bytes().to_vec();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "What is the price of E350?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.5);
    assert!(results[0].node.get_text_content().unwrap().contains("3000"));
}

#[tokio::test]
async fn local_txt_metadata_parsing_test() {
    let input_text = "\
        This is a test content with metadata
        timestamp: {{{timestamp:2024-04-17T23:41:30Z}}}
        Username: {{{username:myCoolUsername}}}

        Main content should remain unaffected.

        Custom {{{metadata-key:metadata-value}}} should be parsed correctly.
        
        Likes: {{{likes:999}}}
        Reposts: {{{reposts:99}}}
        Replies: {{{replies:9}}}

        Invalid metadata values should be ignored. {{{timestamp:br0K3n}}}
        
        Pure metadata should be removed.
        !{{{pg_nums:[19, 20]}}}!
        
        Make this long enough to exceed max node text size and add more metadata.
        Datetime {{{datetime:2000-01-2T02:17:59Z}}} should be parsed too.";

    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "test_input.txt";
    let buffer = input_text.as_bytes().to_vec();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "What is my username?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].node.get_text_content().unwrap().contains("myCoolUsername"));
    assert!(!results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("{{{timestamp:2024-04-17T23:41:30Z}}}"));
    assert!(results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("2024-04-17T23:41:30Z"));
    assert!(results[0].node.metadata.as_ref().unwrap().contains_key("likes"));
    assert!(!results[0].node.get_text_content().unwrap().contains("pg_nums"));
    assert!(results[0].node.metadata.as_ref().unwrap().contains_key("pg_nums"));
    assert_ne!(
        results[0].node.metadata.as_ref().unwrap().get("datetime").unwrap(),
        "br0K3n"
    );

    // Perform another vector search
    let query_string2 = "What is the parsed datetime?".to_string();
    let query_embedding2 = generator.generate_embedding_default(&query_string2).await.unwrap();
    let results2 = resource.as_trait_object().vector_search(query_embedding2, 3);

    assert!(results2[0].node.get_text_content().unwrap().contains("2000"));
    assert!(results2[0].node.metadata.as_ref().unwrap().contains_key("datetime"));
}

#[tokio::test]
async fn local_csv_metadata_parsing_test() {
    let csv_data = "\
        Country,City,Airline,Price
        USA,New York,Delta Airlines,500
        USA,Los Angeles,United Airlines,450
        UK,London,British Airways,{{{price:600}}}
        France,Paris,Air France,550
        Germany,Berlin,Lufthansa,400
        Australia,Sydney,Qantas Airways !{{{carry_pets:true}}}!,700";

    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "input.csv";
    let buffer = csv_data.as_bytes().to_vec();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "What is the price of a London ticket?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.4);
    assert!(results[0].node.get_text_content().unwrap().contains("600"));
    assert!(!results[0].node.get_text_content().unwrap().contains("{{{price:600}}}"));
    assert_eq!(results[0].node.metadata.as_ref().unwrap().get("price").unwrap(), "600");

    // Perform another vector search
    let query_string2 = "Which airline goes to Sydney?".to_string();
    let query_embedding2 = generator.generate_embedding_default(&query_string2).await.unwrap();
    let results2 = resource.as_trait_object().vector_search(query_embedding2, 3);

    assert!(results2[0].score > 0.4);
    assert!(results2[0].node.get_text_content().unwrap().contains("Qantas"));
    assert!(!results2[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("!{{{carry_pets:true}}}!"));
    assert_eq!(
        results2[0].node.metadata.as_ref().unwrap().get("carry_pets").unwrap(),
        "true"
    );
}

// #[tokio::test]
async fn local_md_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "parsed_channels.md";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, false, false);

    // Perform vector search
    let query_string = "What is happening on OpenSea?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.7);
    assert!(results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("KoL Token has successfully launched"));
    assert_eq!(results[0].node.metadata.as_ref().unwrap().len(), 9);
    assert_eq!(results[0].node.metadata.as_ref().unwrap().get("recasts").unwrap(), "0");
    assert_eq!(
        results[0].node.metadata.as_ref().unwrap().get("hash").unwrap(),
        "0x43b9a4bc24246855e3d5f4459a7a3d79e50505e6"
    );

    // Get Farcaster Posts
    let query_string = "Get Farcaster Posts".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results.iter().all(|node| node.score > 0.6));
    assert!(results
        .iter()
        .any(|node| node.node.get_text_content().unwrap().contains("Pepe Runner 2049")));
    assert!(results
        .iter()
        .any(|node| node.node.get_text_content().unwrap().contains("i never remember")));
    assert!(results
        .iter()
        .any(|node| node.node.get_text_content().unwrap().contains("wowow.shibuya.xyz")));

    // Shinkai Vector Resources
    let query_string = "Explain Shinkai Vector Resources".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.6);
    assert!(results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("A powerful native Rust"));

    // Test URL metadata parsing
    let query_string = "How to import into project?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert_eq!(
        results[0].node.metadata.as_ref().unwrap().get("link-urls").unwrap(),
        &serde_json::to_string(&vec!["[desktop-only](https://www.shinkai.com/)"]).unwrap()
    );
    assert!(results[0]
        .node
        .metadata
        .as_ref()
        .unwrap()
        .get("image-urls")
        .unwrap()
        .contains("WebAssembly_Logo.svg"));
    assert!(!results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("WebAssembly_Logo.svg"));
}

// #[tokio::test]
async fn local_html_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "sample.html";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    // Perform vector search
    let query_string = "Explain Benefits of AI in Video Processing".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.8);
    assert!(results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("Improved video analysis"));

    // Test URL metadata parsing
    let query_string = "Video Processing Solutions with AI".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.7);
    assert!(results.iter().any(|node| node
        .node
        .metadata
        .as_ref()
        .unwrap_or(&HashMap::new())
        .get("image-urls")
        .unwrap_or(&String::new())
        .contains("Video Processing")));
    assert!(results.iter().any(|node| node
        .node
        .metadata
        .as_ref()
        .unwrap_or(&HashMap::new())
        .get("link-urls")
        .unwrap_or(&String::new())
        .contains("AI Video")));
}

#[tokio::test]
async fn local_docx_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "decision_log.docx";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, false, false);

    // Perform vector search
    let query_string = "What does this document track?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results[0].score > 0.5);
    assert!(results[0]
        .node
        .get_text_content()
        .unwrap()
        .contains("open and finalized decisions"));
}

#[tokio::test]
async fn local_json_parsing_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "echo_definition.json";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Local,
    )
    .await
    .unwrap();

    resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, false, false);

    // Perform vector search
    let query_string = "Shinkai echo description".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    assert!(results.iter().all(|node| node.score > 0.6));
    assert!(results.iter().any(|node| node
        .node
        .get_text_content()
        .unwrap()
        .contains("Echoes the input message")));
}

#[tokio::test]
async fn metadata_parsing_with_unstructured_test() {
    let generator = RemoteEmbeddingGenerator::new_default();
    let source_file_name = "parsed_channels.md";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).unwrap();
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer,
        &generator,
        source_file_name.to_string(),
        None,
        &vec![],
        generator.model_type().max_input_token_count() as u64,
        DistributionInfo::new_empty(),
        FileParser::Unstructured(UnstructuredAPI::new_default()),
    )
    .await
    .unwrap();

    resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, false, false);

    // Perform vector search
    let query_string = "Trading Party on OpenSea".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let results = resource.as_trait_object().vector_search(query_embedding, 3);

    let node = &results
        .iter()
        .find(|node| {
            node.node
                .get_text_content()
                .unwrap()
                .contains("KoL Token has successfully launched")
        })
        .unwrap()
        .node;

    // Verify templates in text are replaced
    assert!(node
        .get_text_content()
        .unwrap()
        .contains("Timestamp: 2024-04-15T18:32:45+00:00"));
    assert!(node.get_text_content().unwrap().contains("Recasts: 0"));

    // Verify metadata is added
    assert_eq!(
        node.metadata.as_ref().unwrap().get("datetime").unwrap(),
        "2024-04-15T18:32:45+00:00"
    );
    assert_eq!(node.metadata.as_ref().unwrap().get("recasts").unwrap(), "0");
    assert_eq!(
        node.metadata.as_ref().unwrap().get("hash").unwrap(),
        "0x43b9a4bc24246855e3d5f4459a7a3d79e50505e6"
    );
}
