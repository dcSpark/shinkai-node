fn main() {
    println!("cargo:warning=OUT_DIR is: {:?}", std::env::var("OUT_DIR"));
}
