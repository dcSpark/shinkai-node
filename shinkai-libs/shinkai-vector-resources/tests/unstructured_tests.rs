use lazy_static::lazy_static;
use serde_json::Value as JsonValue;
use shinkai_vector_resources::unstructured::*;
use std::fs;

lazy_static! {
    // pub static ref UNSTRUCTURED_API_URL: &'static str =
    //     "https://internal.shinkai.com/x-unstructured-api/general/v0/general";
    pub static ref UNSTRUCTURED_API_URL: &'static str = "http://34.41.225.139/x-unstructured-api/general/v0/general";
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
fn test_unstructured_parse_pdf() {
    let file_path = "../../files/Zeko_Mina_Rollup.pdf";
    let file_name = "Zeko_Mina_Rollup.pdf";

    // Read the file into a byte vector
    let file_buffer = fs::read(file_path).unwrap();

    // Create an UnstructuredAPI and process the file
    let api = UnstructuredAPI::new(UNSTRUCTURED_API_URL.to_string(), None);
    let elements = api.process_file_request_blocking(file_buffer, file_name).unwrap();

    // Check the parsed elements

    // Print all elements for debugging
    for element in &elements {
        println!("{:?}", element);
    }

    assert_eq!(1, 2);
}
