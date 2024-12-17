use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShinkaiPath {
    path: PathBuf,
}

impl ShinkaiPath {
    /// Creates a new ShinkaiPath from a string slice.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        ShinkaiPath {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Returns the base path from the NODE_STORAGE_PATH environment variable.
    fn base_path() -> PathBuf {
        env::var("NODE_STORAGE_PATH")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("storage"))
    }

    /// Creates a new ShinkaiPath from a String, prepending the base path if necessary.
    pub fn from_string(path: String) -> Self {
        let base_path = Self::base_path();
        let path_buf = PathBuf::from(&path);

        // Check if the path is already absolute
        if path_buf.is_absolute() {
            ShinkaiPath { path: path_buf }
        } else {
            ShinkaiPath {
                path: base_path.join(path_buf),
            }
        }
    }

    /// Returns the path as a string slice.
    pub fn as_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }

    /// Checks if the path exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Appends a component to the path.
    pub fn push<P: AsRef<Path>>(&mut self, component: P) {
        self.path.push(component);
    }

    /// Returns the parent directory of the path, if it exists.
    pub fn parent(&self) -> Option<ShinkaiPath> {
        self.path.parent().map(|p| ShinkaiPath::new(p))
    }

    /// Converts the ShinkaiPath to a Path reference.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// Implement Display for ShinkaiPath to easily print it
impl fmt::Display for ShinkaiPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// Add tests for the new functionality
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_base_path() {
        env::set_var("NODE_STORAGE_PATH", "/Users/Nico/my_path");
        assert_eq!(
            ShinkaiPath::base_path(),
            PathBuf::from("/Users/Nico/my_path")
        );
        env::remove_var("NODE_STORAGE_PATH");
    }

    #[test]
    #[serial]
    fn test_from_string_with_base_path() {
        env::set_var("NODE_STORAGE_PATH", "/Users/Nico/my_path");
        assert_eq!(env::var("NODE_STORAGE_PATH").unwrap(), "/Users/Nico/my_path");

        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(
            path.as_path(),
            Path::new("/Users/Nico/my_path/word_files/christmas.docx")
        );

        env::remove_var("NODE_STORAGE_PATH");
    }

    #[test]
    #[serial]
    fn test_from_string_without_base_path() {
        env::remove_var("NODE_STORAGE_PATH");
        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(
            path.as_path(),
            Path::new("storage/word_files/christmas.docx")
        );
    }
}
