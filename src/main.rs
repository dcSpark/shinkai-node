// main.rs

extern crate serde_json;

mod message {
    include!(concat!(env!("OUT_DIR"), "/message.rs"));
}

use message::{Message as ProtoMessage, Body, Field, InternalMetadata, MessageSchemaType, Topic, ExternalMetadata};
use prost::Message;
use serde_json::Value;

#[tokio::main]
async fn main() {
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
                },
                "encryption": "AES",
                "external_metadata": {
                    "sender": "Alice",
                    "recipient": "Bob",
                    "scheduled_time": "2023-12-01T00:00:00Z",
                    "signature": "ABC123"
                }
            }
        }
    }"#;
    print!("JSON string: {}", json_string);

    let json_value: Value = serde_json::from_str(json_string).unwrap();
    let fields = &json_value["message"]["body"]["internal_metadata"]["message_schema_type"]["fields"];
    let mut fields_vec = Vec::new();
    for i in 0..fields.as_array().unwrap().len() {
        fields_vec.push(Field {
            name: fields[i]["name"].as_str().unwrap().to_string(),
            r#type: fields[i]["type"].as_str().unwrap().to_string(),
        });
    }

    let message_schema = MessageSchemaType {
        type_name: json_value["message"]["body"]["internal_metadata"]["message_schema_type"]["type_name"].as_str().unwrap().to_string(),
        fields: fields_vec,
    };

    let topic = Topic {
        topic_id: json_value["message"]["body"]["internal_metadata"]["topic"]["topic_id"].as_str().unwrap().to_string(),
        channel_id: json_value["message"]["body"]["internal_metadata"]["topic"]["channel_id"].as_str().unwrap().to_string(),
    };

    let message = ProtoMessage {
        body: Some(Body {
            content: json_value["message"]["body"]["content"].as_str().unwrap().to_string(),
            internal_metadata: Some(InternalMetadata {
                message_schema_type: Some(message_schema),
                topic: Some(topic),
                content: json_value["message"]["body"]["internal_metadata"]["content"].as_str().unwrap().to_string(),
            }),
            encryption: json_value["message"]["body"]["encryption"].as_str().unwrap().to_string(),
            external_metadata: Some(ExternalMetadata {
                sender: json_value["message"]["body"]["external_metadata"]["sender"].as_str().unwrap().to_string(),
                recipient: json_value["message"]["body"]["external_metadata"]["recipient"].as_str().unwrap().to_string(),
                scheduled_time: json_value["message"]["body"]["external_metadata"]["scheduled_time"].as_str().unwrap().to_string(),
                signature: json_value["message"]["body"]["external_metadata"]["signature"].as_str().unwrap().to_string(),
            }),
        }),
    };

    let mut bytes = Vec::new();
    message.encode(&mut bytes).unwrap();

    println!("Encoded message: {:?}", bytes);

    // Decoding the message
    let decoded_message = ProtoMessage::decode(bytes.as_slice()).unwrap();
    println!("Decoded message: {:?}", decoded_message);
}
