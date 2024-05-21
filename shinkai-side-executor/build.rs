// === Static linking ===
// PDFIUM_STATIC_LIB_PATH="/path-to/shinkai-node/shinkai-side-executor/pdfium" cargo build

fn main() {
    // Nedeed only for macOS
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
}
