use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};

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
