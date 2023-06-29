pub mod network;
pub mod shinkai_message;
pub mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}
