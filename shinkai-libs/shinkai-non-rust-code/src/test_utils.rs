// Duplicated code. Find a way to share it.

#[cfg(test)]

/// Create a temporary directory and set the NODE_STORAGE_PATH environment variable
/// Return the TempDir object (required so it doesn't get deleted when the function returns)
pub fn testing_create_tempdir_and_set_env_var() -> tempfile::TempDir {
    use std::env;
    use std::path::PathBuf;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

    env::set_var(
        "SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH",
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/shinkai-tools-runner-resources/deno")
            .to_string_lossy()
            .to_string(),
    );

    env::set_var(
        "SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH",
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/shinkai-tools-runner-resources/uv")
            .to_string_lossy()
            .to_string(),
    );
    dir
}
