use serde_json::Value as JsonValue;
use shinkai_vector_resources::unstructured::*;

#[test]
fn test_parse_response_json() {
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
    let result = UnstructuredResponseParser::parse_response_json(json_value).unwrap();

    assert_eq!(result.len(), 1);
    if let UnstructuredElement::Title(title) = &result[0] {
        assert_eq!(title.element_type, "Title");
        assert_eq!(title.element_id, "c674a556a00e31fb747e81263a3584be");
        assert_eq!(title.metadata.filename, "Zeko_Mina_Rollup.pdf");
        assert_eq!(title.metadata.filetype, "application/pdf");
        assert_eq!(title.metadata.page_number, 1);
        assert_eq!(
            title.text,
            "Zeko: Fractal scaling of ZK applications using a Shared Sequencer L2 Stack"
        );
    } else {
        panic!("Expected a Title element");
    }
}
