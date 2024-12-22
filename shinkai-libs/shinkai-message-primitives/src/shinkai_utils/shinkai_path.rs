use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShinkaiPath {
    path: PathBuf,
}

impl ShinkaiPath {
    /// Private helper method to create a ShinkaiPath from a &str.
    fn new(path: &str) -> Self {
        let base_path = Self::base_path();
        let path_buf = PathBuf::from(path);

        let final_path = if path_buf.is_absolute() {
            path_buf
        } else {
            base_path.join(path_buf)
        };

        ShinkaiPath { path: final_path }
    }

    /// Returns the base path from the NODE_STORAGE_PATH environment variable.
    /// Defaults to "storage" if not set.
    fn base_path() -> PathBuf {
        env::var("NODE_STORAGE_PATH")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("storage"))
    }

    /// Creates a new ShinkaiPath from a string slice, ensuring it's absolute relative to the base path.
    /// If `path` is not absolute, it is joined to the base path.
    pub fn from_str(path: &str) -> Self {
        Self::new(path)
    }

    /// Creates a new ShinkaiPath from a String, ensuring it's absolute relative to the base path.
    /// If `path` is not absolute, it is joined to the base path.
    pub fn from_string(path: String) -> Self {
        Self::new(&path)
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

    /// Converts the ShinkaiPath to a Path reference.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Returns the relative path of this ShinkaiPath with respect to the base path.
    /// If the path is not under the base directory, returns the full path as-is.
    pub fn relative_path(&self) -> &str {
        let base = Self::base_path();
        if let Ok(stripped) = self.path.strip_prefix(&base) {
            stripped.to_str().unwrap_or("")
        } else {
            // If the path does not lie under the base path,
            // you can decide what to do. Here we return the full path string.
            self.as_str()
        }
    }

    /// Returns the extension of the path, if any.
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|ext| ext.to_str())
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
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_base_path() {
        env::set_var("NODE_STORAGE_PATH", "/Users/Nico/my_path");
        assert_eq!(ShinkaiPath::base_path(), PathBuf::from("/Users/Nico/my_path"));
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
        assert_eq!(path.relative_path(), "word_files/christmas.docx");

        env::remove_var("NODE_STORAGE_PATH");
    }

    #[test]
    #[serial]
    fn test_from_string_without_base_path() {
        env::remove_var("NODE_STORAGE_PATH");
        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(path.as_path(), Path::new("storage/word_files/christmas.docx"));
        assert_eq!(path.relative_path(), "word_files/christmas.docx");
    }

    #[test]
    #[serial]
    fn test_relative_path_outside_base() {
        env::set_var("NODE_STORAGE_PATH", "/Users/Nico/my_path");
        let absolute_outside = ShinkaiPath::from_string("/some/other/path".to_string());
        // Not under /Users/Nico/my_path, so relative_path() returns full path.
        assert_eq!(absolute_outside.relative_path(), "/some/other/path");
        env::remove_var("NODE_STORAGE_PATH");
    }

    #[test]
    #[serial]
    fn test_extension() {
        let path_with_extension = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(path_with_extension.extension(), Some("docx"));

        let path_without_extension = ShinkaiPath::from_string("word_files/christmas".to_string());
        assert_eq!(path_without_extension.extension(), None);
    }
}
