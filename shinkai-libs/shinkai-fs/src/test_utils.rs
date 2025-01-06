// Duplicated code. Find a way to share it.

#[cfg(test)]

/// Create a temporary directory and set the NODE_STORAGE_PATH environment variable
/// Return the TempDir object (required so it doesn't get deleted when the function returns)
pub fn testing_create_tempdir_and_set_env_var() -> tempfile::TempDir {
    use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
    use std::env;
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

    let shinkai_path = ShinkaiPath::from_base_path();

    // Check if the directory exists, and create it if it doesn't
    if !shinkai_path.as_path().exists() {
        let _ = fs::create_dir_all(&shinkai_path.as_path()).map_err(|e| {
            eprintln!("Failed to create directory {}: {}", shinkai_path.as_path().display(), e);
            panic!("Failed to create directory {}: {}", shinkai_path.as_path().display(), e);
        });
    }

    dir // Return the TempDir object
}
