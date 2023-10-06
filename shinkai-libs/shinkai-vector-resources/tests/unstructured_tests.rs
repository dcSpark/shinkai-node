use lazy_static::lazy_static;
use serde_json::Value as JsonValue;
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::source::VRSource;
use shinkai_vector_resources::unstructured::*;
use std::fs;
use std::fs::File;
use std::io;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref UNSTRUCTURED_API_URL: &'static str = "https://internal.shinkai.com/";
    pub static ref DEFAULT_LOCAL_EMBEDDINGS_PORT: &'static str = "7999";
}

#[test]
fn test_unstructured_parse_response_json() {
    let json_str = r#"
        [
            {
                "type": "Title",
                "element_id": "c674a556a00e31fb747e81263a3584be",
                "metadata": {
                    "filename": "Zeko_Mina_Rollup.pdf",
                    "filetype": "application/pdf",
                    "page_number": 1
                },
                "text": "Zeko: Fractal scaling of ZK applications using a Shared Sequencer L2 Stack"
            }
        ]
        "#;

    let json_value: JsonValue = serde_json::from_str(json_str).unwrap();
    let result = UnstructuredParser::parse_response_json(json_value).unwrap();

    assert_eq!(result.len(), 1);
    if ElementType::Title == result[0].element_type {
        assert_eq!(result[0].element_id, "c674a556a00e31fb747e81263a3584be");
        assert_eq!(result[0].metadata.filename, "Zeko_Mina_Rollup.pdf");
        assert_eq!(result[0].metadata.filetype, "application/pdf");
        assert_eq!(result[0].metadata.page_number, Some(1));
        assert_eq!(
            result[0].text,
            "Zeko: Fractal scaling of ZK applications using a Shared Sequencer L2 Stack"
        );
    } else {
        panic!("Expected a Title element");
    }
}

#[test]
fn test_unstructured_parse_pdf_vector_resource() {
    let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
    let generator = RemoteEmbeddingGenerator::new_default();

    let file_name = "Zeko_Mina_Rollup.pdf";
    let file_path = "../../files/".to_string() + file_name;

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new(UNSTRUCTURED_API_URL.to_string(), None);

    let resource = api
        .process_file(file_buffer, &generator, file_name, None, VRSource::None, &vec![], 500)
        .unwrap();

    let query_string = "When does a sequencer cross-reference what has already been committed to Zeko?";
    let query_embedding1 = generator.generate_embedding_default(query_string).unwrap();
    let res = resource.as_trait_object().vector_search(query_embedding1.clone(), 5);
    for (i, result) in res.iter().enumerate() {
        println!("Result {}: {}", result.score, result.chunk.get_data_string().unwrap());
    }
    assert_eq!("4.3 Sequencer Generates Queue Of Pending Transfer Requests From the pool smart contract on the DA layer, each sequencer locally generates a queue of pending (to be applied) transfer requests. This is done via a process where the sequencer locally verifies each of the proofs attached to the transfer requests, and cross-references which ones have already been committed Zeko: Recursive zero-knowledge applications, at scale", res[0].chunk.get_data_string().unwrap());
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
