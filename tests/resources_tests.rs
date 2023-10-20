use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_node::agent::file_parsing::ParsingHelper;
use shinkai_node::db::ShinkaiDB;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::{SourceReference, VRSource};
use shinkai_vector_resources::vector_resource::VectorResource;
use std::fs;
use std::path::Path;
use tokio::runtime::Runtime;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap()
}

fn get_shinkai_intro_doc(generator: &RemoteEmbeddingGenerator, data_tags: &Vec<DataTag>) -> DocumentVectorResource {
    // Read the pdf from file into a buffer
    let buffer = std::fs::read("files/shinkai_intro.pdf")
        .map_err(|_| VRError::FailedPDFParsing)
        .unwrap();

    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Use block_on to run the async-based batched embedding generation logic
    let resource = rt
        .block_on(async {
            let desc = "An initial introduction to the Shinkai Network.";
            return ParsingHelper::parse_file_into_resource(
                buffer,
                generator,
                "shinkai_intro.pdf".to_string(),
                Some(desc.to_string()),
                data_tags,
                500,
            )
            .await;
        })
        .unwrap();

    resource.as_document_resource().unwrap()
}

#[test]
fn test_pdf_parsed_document_resource_vector_search() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let doc = get_shinkai_intro_doc(&generator, &vec![]);

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing vector search works
    let query_string = "Who is building Shinkai?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai.com Nicolas Arqueros nico@shinkai.com Introduction",
            res[0].node.get_data_string().unwrap()
        );

    let query_string = "What about up-front costs?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "No longer will we need heavy up-front costs to build apps that allow users to use their money/data to interact with others in an extremely limited experience (while also taking away control from the user), but instead we will build the underlying architecture which unlocks the ability for the user’s various AI agents to go about performing everything they need done and connecting all of their devices/data together.",
            res[0].node.get_data_string().unwrap()
        );

    let query_string = "Does this relate to crypto?";
    let query_embedding = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI-coordinated computing paradigm that takes decentralization and user-privacy seriously while offering native integration into the modern crypto stack. This paradigm is unlocked via developing a novel P2P messaging network, Shinkai, which connects all of their devices together and uses LLM agents as the engine that processes all human input. This node will rival the",
            res[0].node.get_data_string().unwrap()
        );
}

#[test]
fn test_pdf_resource_save_to_db() {
    setup();

    let generator = RemoteEmbeddingGenerator::new_default();

    // Read the pdf from file into a buffer
    let doc = get_shinkai_intro_doc(&generator, &vec![]);

    // Init Database
    let db_path = format!("db_tests/{}", "embeddings");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_resource_router(&profile).unwrap();

    // Save/fetch doc
    let resource = BaseVectorResource::from(doc.clone());
    shinkai_db.save_resource(resource, &profile).unwrap();
    let fetched_doc = shinkai_db
        .get_resource(&doc.reference_string(), &profile)
        .unwrap()
        .as_document_resource()
        .unwrap();

    assert_eq!(doc, fetched_doc);
}

#[test]
fn test_multi_resource_db_vector_search() {
    setup();

    let generator = RemoteEmbeddingGenerator::new_default();

    // Create a doc
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSource::new_uri_ref("animalwildlife.com"),
        "animal_resource",
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding_blocking(
        &generator,
        vec!["Dog".to_string(), "Camel".to_string(), "Seals".to_string()],
    )
    .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embeddings = generator.generate_embedding_default_blocking(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embeddings = generator.generate_embedding_default_blocking(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embeddings = generator.generate_embedding_default_blocking(fact3).unwrap();
    doc.append_data(fact1, None, &fact1_embeddings, &vec![]);
    doc.append_data(fact2, None, &fact2_embeddings, &vec![]);
    doc.append_data(fact3, None, &fact3_embeddings, &vec![]);

    // Read the pdf from file into a buffer
    let doc2 = get_shinkai_intro_doc(&generator, &vec![]);

    // Init Database
    let db_path = format!("db_tests/{}", "embeddings");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_resource_router(&profile).unwrap();

    // Save resources to DB
    let resource1 = BaseVectorResource::from(doc.clone());
    let resource2 = BaseVectorResource::from(doc2.clone());
    shinkai_db.save_resources(vec![resource1, resource2], &profile).unwrap();

    // Animal resource vector search
    let query = generator.generate_embedding_default_blocking("Animals").unwrap();
    let fetched_resources = shinkai_db.vector_search_resources(query, 100, &profile).unwrap();
    let fetched_doc = fetched_resources.get(0).unwrap();
    assert_eq!(&doc.resource_id(), &fetched_doc.as_trait_object().resource_id());

    // Shinkai introduction resource vector search
    let query = generator.generate_embedding_default_blocking("Shinkai").unwrap();
    let fetched_resources = shinkai_db.vector_search_resources(query, 1, &profile).unwrap();
    let fetched_doc = fetched_resources.get(0).unwrap();
    assert_eq!(&doc2.resource_id(), &fetched_doc.as_trait_object().resource_id());

    // Camel Node vector search
    let query = generator.generate_embedding_default_blocking("Camels").unwrap();
    let ret_nodes = shinkai_db.vector_search(query, 10, 10, &profile).unwrap();
    let ret_node = ret_nodes.get(0).unwrap();
    assert_eq!(fact2, &ret_node.node.get_data_string().unwrap());

    // Camel Node vector search
    let query = generator
        .generate_embedding_default_blocking("Does this relate to crypto?")
        .unwrap();
    let ret_nodes = shinkai_db.vector_search(query, 10, 10, &profile).unwrap();
    let ret_node = ret_nodes.get(0).unwrap();
    assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI-coordinated computing paradigm that takes decentralization and user-privacy seriously while offering native integration into the modern crypto stack. This paradigm is unlocked via developing a novel P2P messaging network, Shinkai, which connects all of their devices together and uses LLM agents as the engine that processes all human input. This node will rival the",
            &ret_node.node.get_data_string().unwrap()
        );

    // Camel Node proximity vector search
    let query = generator.generate_embedding_default_blocking("Camel").unwrap();
    let ret_nodes = shinkai_db.vector_search_proximity(query, 10, 2, &profile).unwrap();
    let ret_node = ret_nodes.get(0).unwrap();
    let ret_node2 = ret_nodes.get(1).unwrap();
    let ret_node3 = ret_nodes.get(2).unwrap();
    assert_eq!(fact1, &ret_node.node.get_data_string().unwrap());
    assert_eq!(fact2, &ret_node2.node.get_data_string().unwrap());
    assert_eq!(fact3, &ret_node3.node.get_data_string().unwrap());

    // Animal tolerance range vector search
    let query = generator
        .generate_embedding_default_blocking("Animals that perform actions")
        .unwrap();
    let ret_nodes = shinkai_db
        .vector_search_tolerance_ranged(query, 10, 0.4, &profile)
        .unwrap();

    let ret_node = ret_nodes.get(0).unwrap();
    let ret_node2 = ret_nodes.get(1).unwrap();

    assert_eq!(fact1, &ret_node.node.get_data_string().unwrap());
    assert_eq!(fact2, &ret_node2.node.get_data_string().unwrap());
}

#[test]
fn test_db_syntactic_vector_search() {
    setup();

    let generator = RemoteEmbeddingGenerator::new_default();

    // Manually create a few test tags
    let regex1 = r#"\b[€$¥£][0-9]{1,3}(,[0-9]{3})*(\.[0-9]{2})?\b|\b€[0-9]{1,3}(\.[0-9]{3})*,(0-9{2})?\b"#;
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

    // Gen docs with data tags
    let doc = get_shinkai_intro_doc(&generator, &data_tags);

    // Init Database
    let db_path = format!("db_tests/{}", "embeddings");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_resource_router(&profile).unwrap();

    // Save resources to DB
    let resource1 = BaseVectorResource::from(doc);
    shinkai_db.save_resources(vec![resource1], &profile).unwrap();

    // println!("Doc data tag index: {:?}", doc.data_tag_index());

    // Email syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Fetch me emails.")
        .unwrap();
    let fetched_data = shinkai_db
        .syntactic_vector_search(query, 1, 10, &vec![email_tag.name.clone()], &profile)
        .unwrap();
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!("1", &fetched_node.node.id);
    assert!(fetched_data.len() == 1);

    // Multiplier syntactic vector search
    let query = generator
        .generate_embedding_default_blocking("Fetch me multipliers.")
        .unwrap();
    let fetched_data = shinkai_db
        .syntactic_vector_search(query, 1, 10, &vec![multiplier_tag.name.clone()], &profile)
        .unwrap();
    let fetched_node = fetched_data.get(0).unwrap();
    assert_eq!("12", &fetched_node.node.id);
    assert!(fetched_data.len() == 1);
}
