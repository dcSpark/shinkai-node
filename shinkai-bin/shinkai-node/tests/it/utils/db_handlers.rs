use std::fs;
use std::path::Path;

pub fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}