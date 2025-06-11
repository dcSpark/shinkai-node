use std::fs::File;
use std::path::PathBuf;

use shinkai_tools_runner::copy_assets::DENO_VERSION;
use zip::ZipArchive;

fn main() {
    // Path where the build script expects the cached Deno archive
    let resources_dir = PathBuf::from("./").join("shinkai-tools-runner-resources");
    let zip_path = resources_dir.join(format!("deno-{}.zip", DENO_VERSION));

    if zip_path.exists() {
        // Validate that the existing archive is a valid ZIP file
        let valid_zip = File::open(&zip_path)
            .and_then(|f| {
                ZipArchive::new(f)
                    .map(|_| ())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            })
            .is_ok();
        if !valid_zip {
            println!(
                "invalid cached Deno archive detected at {}, removing",
                zip_path.display()
            );
            std::fs::remove_file(&zip_path)
                .expect("failed to remove corrupted Deno archive");
        }
    }

    shinkai_tools_runner::copy_assets::copy_assets(
        Some(PathBuf::from("./")),
        Some(PathBuf::from("../../target").join(std::env::var("PROFILE").unwrap())),
    )
    .expect("failed to copy shinkai-tools-runner assets");
}
