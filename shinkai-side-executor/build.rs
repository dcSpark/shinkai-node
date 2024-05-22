// === Static linking ===
// PDFIUM_STATIC_LIB_PATH="$(pwd)/pdfium" cargo build

fn main() {
    // Nedeed only for macOS
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
}
