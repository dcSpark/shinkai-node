use std::path::PathBuf;

fn main() {
    shinkai_tools_runner::copy_assets::copy_assets(
        Some(PathBuf::from("./")),
        Some(PathBuf::from("../../target").join(std::env::var("PROFILE").unwrap())),
    )
    .expect("failed to copy shinkai-tools-runner assets");
}
