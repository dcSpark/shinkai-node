use crate::message::{
    Body, ExternalMetadata, Field, InternalMetadata, Message as ProtoMessage, MessageSchemaType,
    Topic,
};
pub struct ShinkaiMessageBuilder {
    body: Option<Body>,
    message_schema_type: Option<MessageSchemaType>,
    topic: Option<Topic>,
    internal_metadata_content: Option<String>,
    external_metadata: Option<ExternalMetadata>,
    encryption: Option<String>,
}

impl ShinkaiMessageBuilder {
    pub fn new() -> Self {
        Self {
            body: None,
            message_schema_type: None,
            topic: None,
            internal_metadata_content: None,
            external_metadata: None,
            encryption: None,
        }
    }

    pub fn body(mut self, content: String, encryption: String) -> Self {
        self.body = Some(Body { 
            content,
            encryption,
            internal_metadata: None,
            external_metadata: None,
        });
        self
    }

    pub fn message_schema_type(mut self, type_name: String, fields: Vec<Field>) -> Self {
        self.message_schema_type = Some(MessageSchemaType {
            type_name,
            fields,
        });
        self
    }

    pub fn topic(mut self, topic_id: String, channel_id: String) -> Self {
        self.topic = Some(Topic {
            topic_id,
            channel_id,
        });
        self
    }

    pub fn internal_metadata_content(mut self, content: String) -> Self {
        self.internal_metadata_content = Some(content);
        self
    }

    pub fn external_metadata(mut self, sender: String, recipient: String, scheduled_time: String, signature: String) -> Self {
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
        });
        self
    }

    pub fn build(self) -> Result<ProtoMessage, &'static str> {
        if let Some(body) = self.body {
            let internal_metadata = InternalMetadata {
                message_schema_type: self.message_schema_type,
                topic: self.topic,
                content: self.internal_metadata_content.unwrap_or_else(|| String::from("")),
            };
            let body = Body {
                internal_metadata: Some(internal_metadata),
                external_metadata: self.external_metadata,
                ..body
            };
            Ok(ProtoMessage {
                body: Some(body),
            })
        } else {
            Err("Missing fields")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_with_all_fields() {
        let fields = vec![
            Field {
                name: "field1".to_string(),
                r#type: "type1".to_string(),
            },
            // more fields...
        ];

        let message_result = ShinkaiMessageBuilder::new()
            .body("body content".to_string(), "encryption".to_string())
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(
                "sender".to_string(),
                "recipient".to_string(),
                "scheduled_time".to_string(),
                "signature".to_string(),
            )
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let body = message.body.as_ref().unwrap();
        assert_eq!(body.content, "body content");
        assert_eq!(body.encryption, "encryption");
        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "internal metadata content");
        let external_metadata = body.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, "sender");
    }

    #[test]
    fn test_builder_missing_fields() {
        let message_result = ShinkaiMessageBuilder::new().build();
        assert!(message_result.is_err());
    }
}

