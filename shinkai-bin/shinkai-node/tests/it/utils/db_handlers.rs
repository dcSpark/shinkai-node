use std::fs;
use std::path::{Path, PathBuf};

use shinkai_sqlite::SqliteManager;
use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use tempfile::NamedTempFile;

pub fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);

    let lance_path = Path::new("lance_db_tests/");
    let _ = fs::remove_dir_all(lance_path);

    let lance_path = Path::new("lancedb_tests/");
    let _ = fs::remove_dir_all(lance_path);
}

pub fn setup_test_db() -> SqliteManager {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = PathBuf::from(temp_file.path());
    let api_url = String::new();
    let model_type =
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

    SqliteManager::new(db_path, api_url, model_type).unwrap()
}
