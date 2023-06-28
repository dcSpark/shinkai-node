fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();
}
