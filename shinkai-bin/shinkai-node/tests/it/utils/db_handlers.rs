use std::fs;
use std::path::{Path, PathBuf};

use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;
use tempfile::NamedTempFile;

pub fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

pub fn setup_test_db() -> SqliteManager {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = PathBuf::from(temp_file.path());
    let api_url = String::new();
    let model_type =
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

    SqliteManager::new(db_path, api_url, model_type).unwrap()
}

pub fn setup_node_storage_path() {
    let temp_file = NamedTempFile::new().unwrap();
    eprintln!("Temp file path: {:?}", temp_file.path());

    let path = PathBuf::from(temp_file.path());
    let parent_path = path.parent().unwrap();

    std::env::set_var("NODE_STORAGE_PATH", parent_path);

    let base_path = ShinkaiPath::base_path();

    eprintln!("Base path: {:?}", base_path.as_path());

    // Ensure the directory is empty
    let _ = fs::remove_dir_all(base_path.as_path());
}
