fn main() {
    // Nedeed only for macOS
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
}
