use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json;
use std::env;
use std::fmt;
use std::hash::Hash;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShinkaiPath {
    pub path: PathBuf,
}

impl ShinkaiPath {
    /// Private helper method to create a ShinkaiPath from a &str.
    pub fn new(path: &str) -> Self {
        let base_path = Self::base_path();
        let path_buf = os_path::OsPath::from(path).to_pathbuf(); // PathBuf::from(path);

        let final_path = if path_buf.is_absolute() {
            if path_buf.starts_with(&base_path) {
                path_buf
            } else {
                base_path.join(path_buf.strip_prefix("/").unwrap_or(&path_buf))
            }
        } else {
            // Check if base_path is part of path_buf
            if path_buf.starts_with(&base_path) {
                path_buf
            } else {
                base_path.join(path_buf)
            }
        };

        ShinkaiPath { path: final_path }
    }

    /// Returns the base path from the NODE_STORAGE_PATH environment variable,
    /// joined with "filesystem". Defaults to "storage/filesystem" if not set.
    pub fn base_path() -> PathBuf {
        env::var("NODE_STORAGE_PATH")
            .ok()
            .map(|p| PathBuf::from(p).join("filesystem"))
            .unwrap_or_else(|| PathBuf::from("storage/filesystem"))
    }

    /// Creates a new ShinkaiPath from a string slice, ensuring it's absolute relative to the base path.
    /// If `path` is not absolute, it is joined to the base path.
    pub fn from_str(path: &str) -> Self {
        Self::new(path)
    }

    /// Creates a new ShinkaiPath from a String, ensuring it's absolute relative to the base path.
    /// If `path` is not absolute, it is joined to the base path.
    /// Note: This doesn't check if the path exists.
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

    /// Returns the full path as a string slice.
    pub fn full_path(&self) -> &str {
        self.as_str()
    }

    /// Returns the base path as a String.
    pub fn base_path_as_string() -> String {
        Self::base_path().to_str().unwrap_or("").to_string()
    }

    /// Creates a new ShinkaiPath representing the base path.
    pub fn from_base_path() -> Self {
        Self::new("")
    }

    /// Checks if the path is a file.
    pub fn is_file(&self) -> bool {
        self.path.is_file()
    }

    /// Returns the filename with its extension, if any, and if it's not a directory.
    pub fn filename(&self) -> Option<&str> {
        if self.is_file() {
            self.path.file_name().and_then(|name| name.to_str())
        } else {
            None
        }
    }

    /// Returns the parent directory as a new ShinkaiPath, if it exists.
    pub fn parent(&self) -> Option<ShinkaiPath> {
        self.path.parent().map(|p| ShinkaiPath::new(p.to_str().unwrap()))
    }
}

// Implement Display for ShinkaiPath to easily print it
impl fmt::Display for ShinkaiPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShinkaiPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ShinkaiPathVisitor;

        impl<'de> Visitor<'de> for ShinkaiPathVisitor {
            type Value = ShinkaiPath;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("either a string or an object with a `path` field")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ShinkaiPath::from_str(value))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut path_field = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "path" => {
                            if path_field.is_some() {
                                return Err(de::Error::duplicate_field("path"));
                            }
                            path_field = Some(map.next_value()?);
                        }
                        _ => {
                            let _ignored: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let actual_path: String = path_field.ok_or_else(|| de::Error::missing_field("path"))?;
                Ok(ShinkaiPath::from_str(&actual_path))
            }
        }

        // deserialize_any will check the JSON token and call visit_str or visit_map accordingly
        deserializer.deserialize_any(ShinkaiPathVisitor)
    }
}

impl Serialize for ShinkaiPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the relative path as a string
        serializer.serialize_str(self.relative_path())
    }
}

// Add tests for the new functionality
#[cfg(test)]
mod tests {
    use crate::shinkai_utils::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;
    use serial_test::serial;
    use std::{env, fs};

    #[test]
    #[serial]
    fn test_base_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        assert_eq!(
            ShinkaiPath::base_path(),
            PathBuf::from(env::var("NODE_STORAGE_PATH").unwrap()).join("filesystem")
        );
    }

    #[test]
    #[serial]
    fn test_from_string_with_base_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(
            path.as_path(),
            Path::new(&format!(
                "{}/filesystem/word_files/christmas.docx",
                env::var("NODE_STORAGE_PATH").unwrap()
            ))
        );
        assert_eq!(path.relative_path(), os_path::OsPath::from("word_files/christmas.docx").to_string());
    }

    #[test]
    #[serial]
    fn test_from_string_without_base_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        env::remove_var("NODE_STORAGE_PATH");
        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(
            path.as_path(),
            Path::new("storage/filesystem/word_files/christmas.docx")
        );
        assert_eq!(path.relative_path(), os_path::OsPath::from("word_files/christmas.docx").to_string());
    }

    #[test]
    #[serial]
    fn test_relative_path_outside_base() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let absolute_outside = ShinkaiPath::from_string("/some/other/path".to_string());
        assert_eq!(absolute_outside.relative_path(), os_path::OsPath::from("some/other/path").to_string());
    }

    #[test]
    #[serial]
    fn test_extension() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let path_with_extension = ShinkaiPath::from_string("word_files/christmas.docx".to_string());
        assert_eq!(path_with_extension.extension(), Some("docx"));

        let path_without_extension = ShinkaiPath::from_string("word_files/christmas".to_string());
        assert_eq!(path_without_extension.extension(), None);
    }

    #[test]
    #[serial]
    fn test_new_with_base_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let base_path = ShinkaiPath::base_path();
        eprintln!("base_path: {:?}", base_path);
        let test_path = base_path.join("some/relative/path");
        eprintln!("test_path: {:?}", test_path);
        let shinkai_path = ShinkaiPath::new(test_path.to_str().unwrap());
        eprintln!("shinkai_path: {:?}", shinkai_path.full_path());
        assert_eq!(shinkai_path.path, test_path);
    }

    #[test]
    #[serial]
    fn test_new_without_base_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let base_path = ShinkaiPath::base_path();
        let relative_path = "some/relative/path";
        let expected_path = base_path.join(relative_path);
        let shinkai_path = ShinkaiPath::new(relative_path);
        eprintln!("shinkai_path: {:?}", shinkai_path.full_path());
        eprintln!("expected_path: {:?}", expected_path);

        assert_eq!(shinkai_path.path, expected_path);
    }

    #[test]
    #[serial]
    fn test_new_with_root_path() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let root_path = "/";
        let shinkai_path = ShinkaiPath::new(root_path);

        let expected_path = ShinkaiPath::base_path().join(root_path.trim_start_matches('/'));
        assert_eq!(shinkai_path.path, expected_path);
    }

    #[test]
    #[serial]
    fn test_is_file() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let file_path = "test_file.txt";
        let shinkai_path = ShinkaiPath::from_string(file_path.to_string());

        fs::create_dir_all(shinkai_path.as_path().parent().unwrap()).unwrap();
        fs::write(shinkai_path.as_path(), "test".as_bytes()).unwrap();

        assert!(shinkai_path.is_file());
    }

    #[test]
    #[serial]
    fn test_filename() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Create a file to test the filename method
        let path_with_extension = "word_files/christmas.docx";
        let shinkai_path_with_extension = ShinkaiPath::from_string(path_with_extension.to_string());
        fs::create_dir_all(shinkai_path_with_extension.as_path().parent().unwrap()).unwrap();
        fs::write(shinkai_path_with_extension.as_path(), "test".as_bytes()).unwrap();
        assert_eq!(shinkai_path_with_extension.filename(), Some("christmas.docx"));

        // Create a file without an extension
        let path_without_extension = "word_files/christmas";
        let shinkai_path_without_extension = ShinkaiPath::from_string(path_without_extension.to_string());
        fs::write(shinkai_path_without_extension.as_path(), "test".as_bytes()).unwrap();
        assert_eq!(shinkai_path_without_extension.filename(), Some("christmas"));

        // Test a directory path
        let path_with_no_filename = "word_files/";
        let shinkai_path_with_no_filename = ShinkaiPath::from_string(path_with_no_filename.to_string());
        assert_eq!(shinkai_path_with_no_filename.filename(), None);
    }

    #[test]
    #[serial]
    fn test_serialize_relative_path() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Create a ShinkaiPath instance
        let path = ShinkaiPath::from_string("word_files/christmas.docx".to_string());

        // Serialize the ShinkaiPath
        let serialized_path = serde_json::to_string(&path).unwrap();

        // Check if the serialized output matches the expected relative path
        let serialized_path_str = serde_json::to_string(&os_path::OsPath::from("word_files/christmas.docx").to_string()).unwrap();

        assert_eq!(serialized_path, serialized_path_str);
    }
}
