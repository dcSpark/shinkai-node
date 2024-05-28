// === Static linking ===
// PDFIUM_STATIC_LIB_PATH="path-to/libpdfium-parent-directory" cargo build

fn main() {
    // Nedeed only for macOS
    //println!("cargo:rustc-link-lib=framework=CoreGraphics");

//    println!("cargo:rustc-link-lib=static=pdfium");
//    println!("cargo:rustc-link-search=native=e:\\Projects\\shinkai-node\\shinkai-side-executor\\pdfium\\win-x64");
}
