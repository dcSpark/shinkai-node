use lazy_static::lazy_static;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::vector_resource::VectorResource;
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
    let query_embedding = generator.generate_embedding_default(query_string).unwrap();
    let res = doc.vector_search(query_embedding.clone(), 1);
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
        Some("veryrealtechfacts.com"),
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
        Some("ostensiblyrealfruitfacts.com"),
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
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact1, res[0].chunk.get_data_string().unwrap());

    // Perform a vector search for data 1 level lower in the tech map resource
    let query_string = "What can I use to access the internet?";
    let query_embedding = generator.generate_embedding_default(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 5);
    assert_eq!(fact4, res[0].chunk.get_data_string().unwrap());

    // Perform a vector search on the fruit doc
    // for data on the base level
    let query_string = "What fruit has its own packaging?";
    let query_embedding = generator.generate_embedding_default(query_string).unwrap();
    let res = fruit_doc.vector_search(query_embedding, 10);

    assert_eq!(fact6, res[0].chunk.get_data_string().unwrap());
}
