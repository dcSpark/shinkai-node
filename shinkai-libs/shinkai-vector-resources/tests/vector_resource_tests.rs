use lazy_static::lazy_static;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::source::VRSource;
use shinkai_vector_resources::vector_resource::{DataContent, TraversalMethod, VectorResource};
use std::fs::File;
use std::io;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_LOCAL_EMBEDDINGS_PORT: &'static str = "7999";
}

pub struct BertCPPProcess {
    child: Child,
}

impl BertCPPProcess {
    /// Starts the BertCPP process, which gets killed if the
    /// the `BertCPPProcess` struct gets dropped.
    pub fn start() -> io::Result<BertCPPProcess> {
        let dev_null = if cfg!(windows) {
            File::open("NUL").unwrap()
        } else {
            File::open("/dev/null").unwrap()
        };

        // Wait for for previous tests bert.cpp to close
        let duration = Duration::from_millis(100);
        thread::sleep(duration);

        let child = Command::new("./bert-cpp-server")
            .arg("--model")
            .arg("models/all-MiniLM-L12-v2.bin")
            .arg("--threads")
            .arg("8")
            .arg("--port")
            .arg(format!("{}", DEFAULT_LOCAL_EMBEDDINGS_PORT.to_string()))
            .stdout(Stdio::from(dev_null.try_clone().unwrap())) // Redirect stdout
            .stderr(Stdio::from(dev_null)) // Redirect stderr
            .spawn()?;

        // Wait for for the BertCPP process to boot up/initialize its
        // web server
        let duration = Duration::from_millis(150);
        thread::sleep(duration);

        Ok(BertCPPProcess { child })
    }
}

impl Drop for BertCPPProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(_) => {
                let duration = Duration::from_millis(150);
                thread::sleep(duration);
                println!("Successfully killed the bert-cpp server process.")
            }
            Err(e) => println!("Failed to kill the bert-cpp server process: {}", e),
        }
    }
}

#[test]
fn test_remote_embeddings_generation() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    let dog_embeddings = generator.generate_embedding_default("dog").unwrap();
    let cat_embeddings = generator.generate_embedding_default("cat").unwrap();

    assert_eq!(dog_embeddings, dog_embeddings);
    assert_eq!(cat_embeddings, cat_embeddings);
    assert_ne!(dog_embeddings, cat_embeddings);
}

#[test]
fn test_manual_resource_vector_search() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
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
    doc.update_resource_embedding(&generator, vec!["animal".to_string(), "wild life".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the doc
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embeddings = generator.generate_embedding_default(fact1).unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embeddings = generator.generate_embedding_default(fact2).unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embeddings = generator.generate_embedding_default(fact3).unwrap();
    doc.append_data(fact1, None, &fact1_embeddings, &vec![]);
    doc.append_data(fact2, None, &fact2_embeddings, &vec![]);
    doc.append_data(fact3, None, &fact3_embeddings, &vec![]);

    // Testing JSON serialization/deserialization
    let json = doc.to_json().unwrap();
    let deserialized_doc: DocumentVectorResource = DocumentVectorResource::from_json(&json).unwrap();
    assert_eq!(doc, deserialized_doc);

    // Testing basic vector search works
    let query_string = "What animal barks?";
    let query_embedding1 = generator.generate_embedding_default(query_string).unwrap();
    let res = doc.vector_search(query_embedding1.clone(), 1);
    assert_eq!(fact1, res[0].chunk.get_data_string().unwrap());

    let query_string2 = "What animal is slow?";
    let query_embedding2 = generator.generate_embedding_default(query_string2).unwrap();
    let res2 = doc.vector_search(query_embedding2, 3);
    assert_eq!(fact2, res2[0].chunk.get_data_string().unwrap());

    let query_string3 = "What animal swims in the ocean?";
    let query_embedding3 = generator.generate_embedding_default(query_string3).unwrap();
    let res3 = doc.vector_search(query_embedding3, 2);
    assert_eq!(fact3, res3[0].chunk.get_data_string().unwrap());

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
        .update_resource_embedding(&generator, vec!["technology".to_string(), "phones".to_string()])
        .unwrap();

    // Prepare embeddings + data, then add it to the map resource
    let fact4 = "Phones provide the power of the internet in your pocket.";
    let fact4_embeddings = generator.generate_embedding_default(fact4).unwrap();
    map_resource.insert_kv("some_key", fact4, None, &fact4_embeddings, &vec![]);

    // Insert the document resource into the map resource
    // To allow for this composability we need to convert the doc into a BaseVectorResource
    let doc_resource = BaseVectorResource::from(doc);
    map_resource.insert_vector_resource("doc_key", doc_resource, None);

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
    let fact5_embeddings = generator.generate_embedding_default(fact5).unwrap();
    let fact6 = "Bananas are tasty and come in their own natural packaging.";
    let fact6_embeddings = generator.generate_embedding_default(fact6).unwrap();
    fruit_doc.append_data(fact5, None, &fact5_embeddings, &vec![]);
    fruit_doc.append_data(fact6, None, &fact6_embeddings, &vec![]);

    // Insert the map resource into the fruit doc
    let map_resource = BaseVectorResource::from(map_resource);
    fruit_doc.append_vector_resource(map_resource, None);

    //
    // Perform Vector Search Tests Through All Levels/Resources
    //

    // Perform a vector search for data 2 levels lower in the fruit doc to ensure
    // that vector searches propagate inwards through all resources
    let res = fruit_doc.vector_search(query_embedding1.clone(), 5);
    assert_eq!(fact1, res[0].chunk.get_data_string().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform a vector search for data 1 level lower in the tech map resource
    let query_string = "What can I use to access the internet?";
    let query_embedding = generator.generate_embedding_default(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact4, res[0].chunk.get_data_string().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/some_key", res[0].format_path_to_string());
    assert_eq!(1, res[0].retrieval_path.depth());

    // Perform a vector search on the fruit doc
    // for data on the base level
    let query_string = "What fruit has its own packaging?";
    let query_embedding = generator.generate_embedding_default(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding.clone(), 10);
    assert_eq!(fact6, res[0].chunk.get_data_string().unwrap());
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/2", res[0].format_path_to_string());
    assert_eq!(0, res[0].retrieval_path.depth());

    //
    // Traversal Tests
    //
    // Perform UntilDepth(0) traversal to ensure it is working properly, assert the dog fact1 cant be found
    let res = fruit_doc.vector_search_with_traversal(query_embedding1.clone(), 5, &TraversalMethod::UntilDepth(0));
    assert_ne!(fact1, res[0].chunk.get_data_string().unwrap());
    assert_eq!(0, res[0].retrieval_path.depth());
    // Perform UntilDepth(1) traversal to ensure it is working properly, assert the BaseVectorResource for animals is found (not fact1)
    let res = fruit_doc.vector_search_with_traversal(query_embedding1.clone(), 5, &TraversalMethod::UntilDepth(1));
    assert_eq!(
        "3 Animal Facts",
        res[0]
            .chunk
            .get_data_vector_resource()
            .unwrap()
            .as_trait_object()
            .name()
    );
    // Perform UntilDepth(2) traversal to ensure it is working properly, assert dog fact1 is found at the correct depth
    let res = fruit_doc.vector_search_with_traversal(query_embedding1.clone(), 5, &TraversalMethod::UntilDepth(2));
    assert_eq!(DataContent::Data(fact1.to_string()), res[0].chunk.data);
    // Perform a VRPath test to validate depth & path formatting
    assert_eq!("/3/doc_key/1", res[0].format_path_to_string());
    assert_eq!(2, res[0].retrieval_path.depth());

    // Perform Exhaustive traversal to ensure it is working properly, assert dog fact1 is found at the correct depth
    // By requesting only 1 result, Efficient traversal does not go deeper, while Exhaustive makes it all the way to the bottom
    let res = fruit_doc.vector_search_with_traversal(query_embedding1.clone(), 1, &TraversalMethod::Exhaustive);
    assert_eq!(DataContent::Data(fact1.to_string()), res[0].chunk.data);
}

#[test]
fn test_manual_syntactic_vector_search() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
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
    doc.update_resource_embedding(&generator, vec!["cv".to_string(), "email".to_string()])
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
    let fact1_embeddings = generator.generate_embedding_default(fact1).unwrap();
    let fact2 = "Birthday: 23/03/1980";
    let fact2_embeddings = generator.generate_embedding_default(fact2).unwrap();
    let fact3 = "Previous Accomplishments: Drove $1,500,000 in sales at my previous company, which translate to a 4x improvement compared to when I joined.";
    let fact3_embeddings = generator.generate_embedding_default(fact3).unwrap();
    doc.append_data(fact1, None, &fact1_embeddings, &data_tags);
    doc.append_data(fact2, None, &fact2_embeddings, &data_tags);
    doc.append_data(fact3, None, &fact3_embeddings, &data_tags);

    // println!("Doc data tag index: {:?}", doc.data_tag_index());

    // Email syntactic vector search
    // In Shinkai the LLM Agent would do a Tag Vector Search in node DB to find the email_tag based on user's prompt
    // And then calls syntactic_vector_search to guarantee the data retrieved is of the correct structure/"type"
    let query = generator
        .generate_embedding_default("What is the applicant's email?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 1, &vec![email_tag.name.clone()]);
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!(DataContent::Data(fact1.to_string()), fetched_chunk.chunk.data);

    // Date syntactic vector search
    let query = generator
        .generate_embedding_default("What is the applicant's birthday?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 10, &vec![date_tag.name.clone()]);
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!(DataContent::Data(fact2.to_string()), fetched_chunk.chunk.data);

    // Price syntactic vector search
    let query = generator
        .generate_embedding_default("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 2, &vec![price_tag.name.clone()]);
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!(DataContent::Data(fact3.to_string()), fetched_chunk.chunk.data);

    // Multiplier syntactic vector search
    let query = generator
        .generate_embedding_default("Any notable accomplishments in previous positions?")
        .unwrap();
    let fetched_data = doc.syntactic_vector_search(query, 5, &vec![multiplier_tag.name.clone()]);
    let fetched_chunk = fetched_data.get(0).unwrap();
    assert_eq!(DataContent::Data(fact3.to_string()), fetched_chunk.chunk.data);
}
