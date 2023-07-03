fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();

    // let mut config = prost_build::Config::new();
    // config.type_attribute(".", "#[derive(Clone)]");

    // config.compile_protos(&["protos/shinkai_message_proto.proto"], &["protos/"])
    //     .expect("Failed to compile protos");
}
