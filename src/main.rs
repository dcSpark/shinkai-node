// main.rs

mod shinkai_message;
mod shinkai_message_builder;

mod message {
    include!(concat!(env!("OUT_DIR"), "/message.rs"));
}

use shinkai_message::ShinkaiMessage;

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

    // Encoding
    let encoded_message = ShinkaiMessage::encode_message(json_string);
    println!("Encoded message: {:?}", encoded_message);

    // Decoding
    let decoded_message = ShinkaiMessage::decode_message(encoded_message);
    println!("Decoded message: {:?}", decoded_message);
}
