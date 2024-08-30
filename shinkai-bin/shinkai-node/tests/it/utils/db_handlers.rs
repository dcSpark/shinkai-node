use std::fs;
use std::path::Path;

pub fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);

    let lance_path = Path::new("lance_db_tests/");
    let _ = fs::remove_dir_all(lance_path);

    let lance_path = Path::new("lancedb_tests/");
    let _ = fs::remove_dir_all(lance_path);
}