fn main() {
    prost_build::compile_protos(&["protos/message.proto"], &["protos"]).unwrap();
}
