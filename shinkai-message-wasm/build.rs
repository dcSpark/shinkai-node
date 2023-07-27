fn main() {
    prost_build::compile_protos(&["protos/shinkai_message_proto.proto"], &["protos"]).unwrap();
    println!("cargo:warning=OUT_DIR is: {:?}", std::env::var("OUT_DIR"));
}
