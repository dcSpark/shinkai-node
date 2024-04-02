use lazy_static::lazy_static;
use serde_json::Value as JsonValue;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::unstructured::unstructured_parser::UnstructuredParser;
use shinkai_vector_resources::unstructured::unstructured_types::ElementType;
use std::fs;

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
    let generator = RemoteEmbeddingGenerator::new_default();

    let file_name = "shinkai_intro.pdf";
    let file_path = "../../files/".to_string() + file_name;

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new_default();

    let resource = api
        .process_file_blocking(
            file_buffer,
            &generator,
            file_name.to_string(),
            None,
            VRSourceReference::None,
            &vec![],
            500,
        )
        .unwrap();

    resource.as_trait_object().print_all_nodes_exhaustive(None, true, false);

    // let query_string = "When does a sequencer cross-reference what has already been committed to Zeko?";
    let query_string = "Who are the authors?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = resource.as_trait_object().vector_search(query_embedding1.clone(), 50);
    for (i, result) in res.iter().enumerate() {
        println!(
            "Score {} - Data: {}",
            result.score,
            result.node.get_text_content().unwrap().to_string()
        );
    }
    assert_eq!(
        "Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai.com Nicolas Arqueros",
        res[0].node.get_text_content().unwrap().to_string()
    );
}

#[test]
fn test_unstructured_parse_txt_vector_resource() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let file_name = "canada.txt";
    let file_path = "../../files/".to_string() + file_name;

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new_default();

    let resource = api
        .process_file_blocking(
            file_buffer,
            &generator,
            file_name.to_string(),
            None,
            VRSourceReference::None,
            &vec![],
            200,
        )
        .unwrap();

    resource.as_trait_object().print_all_nodes_exhaustive(None, true, false);

    let query_string = "What are the main metropolitan cities of Canada?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = resource.as_trait_object().vector_search(query_embedding1.clone(), 50);
    for (i, result) in res.iter().enumerate() {
        println!(
            "Score {} - Data: {}",
            result.score,
            result.node.get_text_content().unwrap().to_string()
        );
    }
    assert_eq!(
        " Ottawa and its three largest metropolitan areas are Toronto, Montreal, and Vancouver.",
        res[0].node.get_text_content().unwrap().to_string()
    );
}

#[test]
fn test_unstructured_parse_epub_vector_resource() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let file_name = "test.epub";
    let file_path = "../../files/".to_string() + file_name;

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new_default();

    let resource = api
        .process_file_blocking(
            file_buffer,
            &generator,
            file_name.to_string(),
            None,
            VRSourceReference::None,
            &vec![],
            300,
        )
        .unwrap();

    resource.as_trait_object().print_all_nodes_exhaustive(None, true, false);

    let query_string = "What are the tests in this book?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = resource.as_trait_object().vector_search(query_embedding1.clone(), 50);
    for (i, result) in res.iter().enumerate() {
        println!(
            "Score {} - Data: {}",
            result.score,
            result.node.get_text_content().unwrap().to_string()
        );
    }
    assert_eq!(
        "This document contains tests which are fundamental to the\naccessibility of Reading Systems for users with disabilities. This is\none test book in a suite of EPUBs for testing accessibility; the other\nbooks cover additional fundamental tests as well as advanced tests.",
        res[0].node.get_text_content().unwrap().to_string()
    );
}

#[test]
fn test_unstructured_parse_html_vector_resource() {
    let generator = RemoteEmbeddingGenerator::new_default();

    let file_name = "unstructured.html";
    let file_path = "../../files/".to_string() + file_name;

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new_default();

    let resource = api
        .process_file_blocking(
            file_buffer,
            &generator,
            file_name.to_string(),
            None,
            VRSourceReference::None,
            &vec![],
            300,
        )
        .unwrap();

    resource.as_trait_object().print_all_nodes_exhaustive(None, true, false);

    let query_string = "What is Unstructured?";
    let query_embedding1 = generator.generate_embedding_default_blocking(query_string).unwrap();
    let res = resource.as_trait_object().vector_search(query_embedding1.clone(), 50);
    for (i, result) in res.iter().enumerate() {
        println!(
            "Score {} - Data: {}",
            result.score,
            result.node.get_text_content().unwrap().to_string()
        );
    }
    // Check that it is in either of the first 2 results. Changes slightly based on unstructured updates (like 0.01% of each other's score)
    let first_eq = "The unstructured library aims to simplify and streamline the preprocessing of structured and unstructured documents for downstream tasks. And what that means is no matter where your data is
and no matter what format that data is in, Unstructured’s toolkit will transform and preprocess that data" ==
        res[0].node.get_text_content().unwrap().to_string();
    let second_eq = "The unstructured library aims to simplify and streamline the preprocessing of structured and unstructured documents for downstream tasks. And what that means is no matter where your data is
and no matter what format that data is in, Unstructured’s toolkit will transform and preprocess that data" ==
        res[1].node.get_text_content().unwrap().to_string();

    assert!(first_eq || second_eq);
}
