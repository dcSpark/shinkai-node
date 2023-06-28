fn main() {
    prost_build::compile_protos(&["protos/shinkai_message.proto"], &["protos"]).unwrap();
}
