// shinkai_message.rs

use crate::shinkai_message::{
    Body, ExternalMetadata, Field, InternalMetadata, ShinkaiMessage, MessageSchemaType,
    Topic,
};
use prost::Message;
use serde_json::Value;

pub struct ShinkaiMessageHandler;

impl ShinkaiMessageHandler {
    pub fn encode_message(json_string: &str) -> Vec<u8> {
        let json_value: Value = serde_json::from_str(json_string).unwrap();
        let fields =
            &json_value["message"]["body"]["internal_metadata"]["message_schema_type"]["fields"];
        let mut fields_vec = Vec::new();
        for i in 0..fields.as_array().unwrap().len() {
            fields_vec.push(Field {
                name: fields[i]["name"].as_str().unwrap().to_string(),
                r#type: fields[i]["type"].as_str().unwrap().to_string(),
            });
        }

        let message_schema = MessageSchemaType {
            type_name: json_value["message"]["body"]["internal_metadata"]["message_schema_type"]
                ["type_name"]
                .as_str()
                .unwrap()
                .to_string(),
            fields: fields_vec,
        };

        let topic = Topic {
            topic_id: json_value["message"]["body"]["internal_metadata"]["topic"]["topic_id"]
                .as_str()
                .unwrap()
                .to_string(),
            channel_id: json_value["message"]["body"]["internal_metadata"]["topic"]["channel_id"]
                .as_str()
                .unwrap()
                .to_string(),
        };

        let body = Body {
            content: json_value["message"]["body"]["content"]
                .as_str()
                .unwrap()
                .to_string(),
            internal_metadata: Some(InternalMetadata {
                message_schema_type: Some(message_schema),
                topic: Some(topic),
                content: json_value["message"]["body"]["internal_metadata"]["content"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            }),
        };

        let external_metadata = ExternalMetadata {
            sender: json_value["message"]["external_metadata"]["sender"]
                .as_str()
                .unwrap()
                .to_string(),
            recipient: json_value["message"]["external_metadata"]["recipient"]
                .as_str()
                .unwrap()
                .to_string(),
            scheduled_time: json_value["message"]["external_metadata"]["scheduled_time"]
                .as_str()
                .unwrap()
                .to_string(),
            signature: json_value["message"]["external_metadata"]["signature"]
                .as_str()
                .unwrap()
                .to_string(),
        };

        let message = ShinkaiMessage {
            body: Some(body),
            external_metadata: Some(external_metadata),
            encryption: json_value["message"]["encryption"]
                .as_str()
                .unwrap()
                .to_string(),
        };

        let mut bytes = Vec::new();
        message.encode(&mut bytes).unwrap();

        bytes
    }

    pub fn decode_message(bytes: Vec<u8>) -> ShinkaiMessage {
        ShinkaiMessage::decode(bytes.as_slice()).unwrap()
    }
}

// shinkai_message.rs

#[cfg(test)]
mod tests {
    use crate::ShinkaiMessageHandler;

    #[test]
    fn test_encode_message() {
        let json_string = r#"{
            "message": {
                "body": {
                    "content": "Hello World",
                    "internal_metadata": {
                        "message_schema_type": {
                            "type_name": "MyType",
                            "fields": [
                                {"name": "field1", "type": "type1"},
                                {"name": "field2", "type": "type2"}
                            ]
                        },
                        "topic": {
                            "topic_id": "my_topic",
                            "channel_id": "my_channel"
                        },
                        "content": "InternalContent"
                    }
                },
                "encryption": "AES",
                "external_metadata": {
                    "sender": "Alice",
                    "recipient": "Bob",
                    "scheduled_time": "2023-12-01T00:00:00Z",
                    "signature": "ABC123"
                }
            }
        }"#;
        let encoded_message = ShinkaiMessageHandler::encode_message(json_string);
        assert!(encoded_message.len() > 0); // The result should be a non-empty vector.
    }

    #[test]
    fn test_decode_message() {
        let json_string = r#"{
            "message": {
                "body": {
                    "content": "Hello World",
                    "internal_metadata": {
                        "message_schema_type": {
                            "type_name": "MyType",
                            "fields": [
                                {"name": "field1", "type": "type1"},
                                {"name": "field2", "type": "type2"}
                            ]
                        },
                        "topic": {
                            "topic_id": "my_topic",
                            "channel_id": "my_channel"
                        },
                        "content": "InternalContent"
                    }
                },
                "encryption": "AES",
                "external_metadata": {
                    "sender": "Alice",
                    "recipient": "Bob",
                    "scheduled_time": "2023-12-01T00:00:00Z",
                    "signature": "ABC123"
                }
            }
        }"#;
        let encoded_message = ShinkaiMessageHandler::encode_message(json_string);
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message);

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "InternalContent");
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .type_name,
            "MyType"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .name,
            "field1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .r#type,
            "type1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .name,
            "field2"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .r#type,
            "type2"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().topic_id,
            "my_topic"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().channel_id,
            "my_channel"
        );

        assert_eq!(decoded_message.encryption, "AES");

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, "Alice");
        assert_eq!(external_metadata.recipient, "Bob");
        assert_eq!(external_metadata.scheduled_time, "2023-12-01T00:00:00Z");
        assert_eq!(external_metadata.signature, "ABC123");
    }
}
