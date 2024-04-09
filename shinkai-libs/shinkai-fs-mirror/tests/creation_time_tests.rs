#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::unix::prelude::MetadataExt;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};
    use shinkai_fs_mirror::synchronizer::FilesystemSynchronizer;
    use tempfile::tempdir;

    fn create_temp_file_with_date_in_name(dir: &tempfile::TempDir, file_name: &str, content: &[u8]) -> PathBuf {
        let file_path = dir.path().join(file_name);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();
        file_path
    }

    #[tokio::test]
    async fn test_creation_datetime_extraction_from_file_name() {
        let dir = tempdir().unwrap();
        let file_path = create_temp_file_with_date_in_name(&dir, "20230101_test_file.txt", b"Test content");

        let extracted_date = FilesystemSynchronizer::creation_datetime_extraction(&file_path).unwrap();
        assert_eq!(extracted_date, Some("2023-01-01T00:00:00+00:00".to_string()));
    }

    #[tokio::test]
    async fn test_creation_datetime_extraction_from_folder_name() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("20230224");
        fs::create_dir(&subdir).unwrap();
        let file_path = create_temp_file_with_date_in_name(&tempfile::TempDir::new_in(subdir).unwrap(), "test_file.txt", b"Test content");

        let extracted_date = FilesystemSynchronizer::creation_datetime_extraction(&file_path).unwrap();
        assert_eq!(extracted_date, Some("2023-02-24T00:00:00+00:00".to_string()));
    }

    #[tokio::test]
    async fn test_creation_datetime_extraction_from_folder_name_another_format() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("2023-02-24");
        fs::create_dir(&subdir).unwrap();
        let file_path = create_temp_file_with_date_in_name(&tempfile::TempDir::new_in(subdir).unwrap(), "test_file.txt", b"Test content");

        let extracted_date = FilesystemSynchronizer::creation_datetime_extraction(&file_path).unwrap();
        assert_eq!(extracted_date, Some("2023-02-24T00:00:00+00:00".to_string()));
    }

    #[tokio::test]
    async fn test_creation_datetime_extraction_with_incorrect_metadata_date() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("20230224");
        fs::create_dir(&subdir).unwrap();
        let file_path = create_temp_file_with_date_in_name(&tempfile::TempDir::new_in(subdir).unwrap(), "how_to_reverse_skin_wrinkles_dr_michelle_wong.json", b"Test content");

        // Simulate incorrect metadata date by setting a future modification time
        let future_time = SystemTime::now() + Duration::new(500_000_000, 0);
        let _ = filetime::set_file_mtime(&file_path, filetime::FileTime::from_system_time(future_time));

        let extracted_date = FilesystemSynchronizer::creation_datetime_extraction(&file_path).unwrap();
        assert_eq!(extracted_date, Some("2023-02-24T00:00:00+00:00".to_string()));
    }
}