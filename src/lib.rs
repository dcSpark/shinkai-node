pub mod db;
pub mod managers;
pub mod network;
pub mod resources;
pub mod shinkai_message;
pub mod utils;
pub mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}
