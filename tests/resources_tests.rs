use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::resources::base_vector_resources::BaseVectorResource;
use shinkai_node::resources::document_resource::DocumentVectorResource;
use shinkai_node::resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_node::resources::map_resource::MapVectorResource;
use shinkai_node::resources::resource_errors::VectorResourceError;
use shinkai_node::resources::vector_resource::VectorResource;
use shinkai_node::resources::{bert_cpp::BertCPPProcess, data_tags::DataTag};
use std::fs;
use std::path::Path;

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
        .map_err(|_| VectorResourceError::FailedPDFParsing)
        .unwrap();

    // Generate DocumentVectorResource
    let desc = "An initial introduction to the Shinkai Network.";
    let doc = DocumentVectorResource::parse_pdf(
        &buffer,
        100,
        generator,
        "Shinkai Introduction",
        Some(desc),
        Some("http://shinkai.com"),
        data_tags,
    )
    .unwrap();

    doc
}

#[test]
fn test_remote_embeddings_generation() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    let dog_embeddings = generator.generate_embedding("dog").unwrap();
    let cat_embeddings = generator.generate_embedding("cat").unwrap();

    assert_eq!(dog_embeddings, dog_embeddings);
    assert_eq!(cat_embeddings, cat_embeddings);
    assert_ne!(dog_embeddings, cat_embeddings);
}

#[test]
fn test_manual_document_resource_vector_search() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    //
    // Create a first resource
    //
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        Some("animalwildlife.com"),
        "animal_resource",
    );
    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding(&generator, vec!["animal".to_string(), "wild life".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embeddings = generator.generate_embedding(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embeddings = generator.generate_embedding(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embeddings = generator.generate_embedding(fact3).unwrap();
    doc.append_data(fact1, None, &fact1_embeddings, &vec![]);
    doc.append_data(fact2, None, &fact2_embeddings, &vec![]);
    doc.append_data(fact3, None, &fact3_embeddings, &vec![]);

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing vector search works
    let query_string = "What animal barks?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = doc.vector_search(query_embedding.clone(), 1);
    assert_eq!(fact1, res[0].chunk.get_data_string().unwrap());

    let query_string2 = "What animal is slow?";
    let query_embedding2 = generator.generate_embedding(query_string2).unwrap();
    let res2 = doc.vector_search(query_embedding2, 3);
    assert_eq!(fact2, res2[0].chunk.get_data_string().unwrap());

    let query_string3 = "What animal swims in the ocean?";
    let query_embedding3 = generator.generate_embedding(query_string3).unwrap();
    let res3 = doc.vector_search(query_embedding3, 2);
    assert_eq!(fact3, res3[0].chunk.get_data_string().unwrap());

    //
    // Create a 2nd resource, a MapVectorResource
    //
    let mut map_resource = MapVectorResource::new_empty(
        "Tech Facts",
        Some("A collection of facts about technology"),
        Some("veryrealtechfacts.com"),
        "tech_resource",
    );

    map_resource.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    map_resource
        .update_resource_embedding(&generator, vec!["technology".to_string(), "phones".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the map resource
    let fact4 = "Phones provide the power of the internet in your pocket.";
    let fact4_embeddings = generator.generate_embedding(fact4).unwrap();
    map_resource.insert_kv("some_key", fact4, None, &fact4_embeddings, &vec![]);

    // Insert the document resource into the map resource
    let doc_resource = BaseVectorResource::Document(doc);
    map_resource.insert_vector_resource("doc_key", doc_resource, None);

    //
    // Create a third resource, a DocumentVectorResource about fruits
    //
    let mut fruit_doc = DocumentVectorResource::new_empty(
        "Fruit Facts",
        Some("A collection of facts about fruits"),
        Some("ostensiblyrealfruitfacts.com"),
        "fruit_resource",
    );
    fruit_doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice

    // Prepare embeddings + data, then add it to the fruit doc
    let fact5 = "Apples are sweet and crunchy.";
    let fact5_embeddings = generator.generate_embedding(fact5).unwrap();
    let fact6 = "Bananas are tasty and come in their own natural packaging.";
    let fact6_embeddings = generator.generate_embedding(fact6).unwrap();
    fruit_doc.append_data(fact5, None, &fact5_embeddings, &vec![]);
    fruit_doc.append_data(fact6, None, &fact6_embeddings, &vec![]);

    // Insert the map resource into the fruit doc
    let map_resource = BaseVectorResource::Map(map_resource);
    fruit_doc.append_vector_resource(map_resource, None);

    // Perform a vector search for data 2 levels lower on the fruit doc to ensure
    // that vector searches propagate inwards through all resources
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact1, res[0].chunk.get_data_string().unwrap());

    // Perform a vector search for data 1 level lower on the fruit doc to ensure
    // that vector searches propagate inwards through all resources
    let query_string = "What can I use to access the internet?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact4, res[0].chunk.get_data_string().unwrap());

    // Perform a vector search on the fruit doc
    // for data on the base level
    let query_string = "What fruit has its own packaging?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 10);

    assert_eq!(fact6, res[0].chunk.get_data_string().unwrap());
}

#[test]
fn test_pdf_parsed_document_resource_vector_search() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    let doc = get_shinkai_intro_doc(&generator, &vec![]);

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing vector search works
    let query_string = "Who is building Shinkai?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai.com Nicolas Arqueros nico@shinkai.com July 21, 2023 1 Introduction With LLMs proving themselves to be very capable in performing many of the core computing tasks we manually/programmatically perform every day, we are entering into a new world where an AI coordinated computing paradigm is inevitable.",
            res[0].chunk.get_data_string().unwrap()
        );

    let query_string = "What about up-front costs?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "No longer will we need heavy up front costs to build apps that allow users to use their money/data to interact with others in an extremely limited experience (while also taking away control from the user), but instead we will build the underlying architecture which unlocks the ability for the user s various AI agents to go about performing everything they need done and connecting all of their devices/data together.",
            res[0].chunk.get_data_string().unwrap()
        );

    let query_string = "Does this relate to crypto?";
    let query_embedding = generator.generate_embedding(query_string).unwrap();
    let res = doc.vector_search(query_embedding, 1);
    assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI coordinated computing paradigm that takes decentralization and user privacy seriously while offering native integration into the modern crypto stack.",
            res[0].chunk.get_data_string().unwrap()
        );
}

#[test]
fn test_pdf_resource_save_to_db() {
    setup();
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    // Read the pdf from file into a buffer
    let doc = get_shinkai_intro_doc(&generator, &vec![]);

    // Init Database
    let db_path = format!("db_tests/{}", "embeddings");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_resource_router(&profile).unwrap();

    // Save/fetch doc
    let resource: Box<dyn VectorResource> = Box::new(doc.clone());
    shinkai_db.save_resource(resource, &profile).unwrap();
    let fetched_doc = shinkai_db.get_document(&doc.db_key(), &profile).unwrap();

    assert_eq!(doc, fetched_doc);
}

#[test]
fn test_multi_resource_db_vector_search() {
    setup();
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    // Create a doc
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        Some("animalwildlife.com"),
        "animal_resource",
    );

    doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
    doc.update_resource_embedding(
        &generator,
        vec!["Dog".to_string(), "Camel".to_string(), "Seals".to_string()],
    )
    .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embeddings = generator.generate_embedding(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embeddings = generator.generate_embedding(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embeddings = generator.generate_embedding(fact3).unwrap();
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
    let resource1 = Box::new(doc.clone()) as Box<dyn VectorResource>;
    let resource2 = Box::new(doc2.clone()) as Box<dyn VectorResource>;
    shinkai_db.save_resources(vec![resource1, resource2], &profile).unwrap();

    // Animal resource vector search
    let query = generator.generate_embedding("Animals").unwrap();
    let fetched_resources = shinkai_db.vector_search_resources(query, 100, &profile).unwrap();
    let fetched_doc = fetched_resources.get(0).unwrap();
    assert_eq!(&doc.resource_id(), &fetched_doc.resource_id());

    // Shinkai introduction resource vector search
    let query = generator.generate_embedding("Shinkai").unwrap();
    let fetched_resources = shinkai_db.vector_search_resources(query, 1, &profile).unwrap();
    let fetched_doc = fetched_resources.get(0).unwrap();
    assert_eq!(&doc2.resource_id(), &fetched_doc.resource_id());

    // Camel DataChunk vector search
    let query = generator.generate_embedding("Camels").unwrap();
    let ret_data_chunks = shinkai_db.vector_search_data(query, 10, 10, &profile).unwrap();
    let ret_data_chunk = ret_data_chunks.get(0).unwrap();
    assert_eq!(fact2, &ret_data_chunk.chunk.get_data_string().unwrap());

    // Camel DataChunk vector search
    let query = generator.generate_embedding("Does this relate to crypto?").unwrap();
    let ret_data_chunks = shinkai_db.vector_search_data(query, 10, 10, &profile).unwrap();
    let ret_data_chunk = ret_data_chunks.get(0).unwrap();
    assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI coordinated computing paradigm that takes decentralization and user privacy seriously while offering native integration into the modern crypto stack.",
            &ret_data_chunk.chunk.get_data_string().unwrap()
        );

    // Camel DataChunk proximity vector search
    let query = generator.generate_embedding("Camel").unwrap();
    let ret_data_chunks = shinkai_db
        .vector_search_data_doc_proximity(query, 10, 2, &profile)
        .unwrap();
    let ret_data_chunk = ret_data_chunks.get(0).unwrap();
    let ret_data_chunk2 = ret_data_chunks.get(1).unwrap();
    let ret_data_chunk3 = ret_data_chunks.get(2).unwrap();
    assert_eq!(fact1, &ret_data_chunk.chunk.get_data_string().unwrap());
    assert_eq!(fact2, &ret_data_chunk2.chunk.get_data_string().unwrap());
    assert_eq!(fact3, &ret_data_chunk3.chunk.get_data_string().unwrap());

    // Animal tolerance range vector search
    let query = generator.generate_embedding("Animals that peform actions").unwrap();
    let ret_data_chunks = shinkai_db
        .vector_search_data_tolerance_ranged(query, 10, 0.4, &profile)
        .unwrap();

    let ret_data_chunk = ret_data_chunks.get(0).unwrap();
    let ret_data_chunk2 = ret_data_chunks.get(1).unwrap();

    assert_eq!(fact1, &ret_data_chunk.chunk.get_data_string().unwrap());
    assert_eq!(fact2, &ret_data_chunk2.chunk.get_data_string().unwrap());
}

#[test]
fn test_db_syntactic_vector_search() {
    setup();
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
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
    let resource1 = Box::new(doc.clone()) as Box<dyn VectorResource>;
    shinkai_db.save_resources(vec![resource1], &profile).unwrap();

    // println!("Doc data tag index: {:?}", doc.data_tag_index());

    // Email syntactic vector search
    let query = generator.generate_embedding("Fetch me emails.").unwrap();
    let fetched_data = shinkai_db
        .syntactic_vector_search_data(query, 1, 10, &vec![email_tag.name.clone()], &profile)
        .unwrap();
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!("1", &fetched_chunk.chunk.id);
    assert!(fetched_data.len() == 1);

    // Multiplier syntactic vector search
    let query = generator.generate_embedding("Fetch me multipliers.").unwrap();
    let fetched_data = shinkai_db
        .syntactic_vector_search_data(query, 1, 10, &vec![multiplier_tag.name.clone()], &profile)
        .unwrap();
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!("15", &fetched_chunk.chunk.id);
    assert!(fetched_data.len() == 1);
}
