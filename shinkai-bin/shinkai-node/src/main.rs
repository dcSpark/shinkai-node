// main.rs
#![recursion_limit = "512"]
mod cron_tasks;
mod db;
mod llm_provider;
mod managers;
mod network;
mod payments;
mod planner;
mod runner;
mod schemas;
mod tools;
mod utils;
mod vector_fs;
mod welcome_files;
mod workflows;

use runner::{initialize_node, run_node_tasks};

#[cfg(feature = "console")]
use console_subscriber;

#[tokio::main]
pub async fn main() {
    #[cfg(feature = "console")]
    {
        console_subscriber::init();
        eprintln!("> tokio-console is enabled");
    }

    #[cfg(feature = "dynamic-pdf-parser")]
    let _ = include_and_extract_pdfium();

    let result = initialize_node().await.unwrap();
    let _ = run_node_tasks(result.1, result.2, result.3).await;
}

#[cfg(feature = "dynamic-pdf-parser")]
fn include_and_extract_pdfium() -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;

    #[cfg(target_os = "linux")]
    let pdfium_lib = "libpdfium.so";
    #[cfg(target_os = "linux")]
    let pdfium_bytes = include_bytes!("../../../shinkai-libs/shinkai-ocr/pdfium/linux-x64/libpdfium.so");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let pdfium_lib = "libpdfium.dylib";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let pdfium_bytes = include_bytes!("../../../shinkai-libs/shinkai-ocr/pdfium/mac-arm64/libpdfium.dylib");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let pdfium_lib = "libpdfium.dylib";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let pdfium_bytes = include_bytes!("../../../shinkai-libs/shinkai-ocr/pdfium/mac-x64/libpdfium.dylib");

    #[cfg(target_os = "windows")]
    let pdfium_lib = "pdfium.dll";
    #[cfg(target_os = "windows")]
    let pdfium_bytes = include_bytes!("../../../shinkai-libs/shinkai-ocr/pdfium/win-x64/pdfium.dll");

    let mut file = File::create(pdfium_lib)?;
    file.write_all(pdfium_bytes)?;

    Ok(())
}
