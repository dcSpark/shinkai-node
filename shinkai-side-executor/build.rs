// === Static linking ===
// PDFIUM_STATIC_LIB_PATH="path-to/libpdfium-parent-directory" cargo build

use std::env;

fn main() {
    let os = env::consts::OS;

    if os == "macos" {
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }
}
