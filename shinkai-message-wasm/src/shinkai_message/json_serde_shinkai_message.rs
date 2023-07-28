use serde::{
    de::{self, Deserialize, Deserializer, SeqAccess, Visitor},
    Serialize, Serializer,
};
use std::fmt;
use crate::schemas::shinkai_message::ShinkaiMessage;
use super::shinkai_message_handler::ShinkaiMessageHandler;

pub struct JSONSerdeShinkaiMessage(pub ShinkaiMessage);

impl JSONSerdeShinkaiMessage {
    pub fn new(msg: ShinkaiMessage) -> Self {
        JSONSerdeShinkaiMessage(msg)
    }
}

impl Serialize for JSONSerdeShinkaiMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = ShinkaiMessageHandler::encode_message(self.0.clone());
        serializer.serialize_bytes(&encoded)
    }
}

pub struct JSONSerdeShinkaiMessageVisitor;

impl<'de> Visitor<'de> for JSONSerdeShinkaiMessageVisitor {
    type Value = JSONSerdeShinkaiMessage;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("byte array")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut bytes = vec![];
        while let Some(byte) = seq.next_element()? {
            bytes.push(byte);
        }
        let message = ShinkaiMessageHandler::decode_message(bytes);
        Ok(JSONSerdeShinkaiMessage(message))
    }
}

impl<'de> Deserialize<'de> for JSONSerdeShinkaiMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(JSONSerdeShinkaiMessageVisitor)
    }
}
