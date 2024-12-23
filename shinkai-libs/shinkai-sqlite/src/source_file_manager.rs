// use rusqlite::params;
// use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

// use crate::{errors::SqliteManagerError, SqliteManager};

// impl SqliteManager {
//     pub fn save_source_file_map(
//         &self,
//         source_file_map: &SourceFileMap,
//         resource_id: &str,
//         profile: &ShinkaiName,
//     ) -> Result<(), SqliteManagerError> {
//         let profile_name = profile
//             .get_profile_name_string()
//             .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;
//         let source_files_dir = Self::get_source_files_path().join(Self::get_root_directory_name(resource_id));

//         let mut conn = self.get_connection()?;
//         let tx = conn.transaction()?;

//         // Store the source file contents in the source files directory
//         for (path, source_file) in &source_file_map.map {
//             let file_dir = source_files_dir.join(path.path_ids.join("/"));
//             std::fs::create_dir_all(&file_dir).map_err(|_| SqliteManagerError::FailedFetchingValue)?;

//             match source_file {
//                 SourceFile::Standard(sf) => {
//                     let file_path = file_dir.join(sf.file_name.clone());
//                     std::fs::write(file_path, sf.file_content.clone())
//                         .map_err(|_| SqliteManagerError::FailedFetchingValue)?;

//                     // Store the source file metadata in the database
//                     tx.execute(
//                         "INSERT OR REPLACE INTO source_file_maps
//                             (profile_name, vector_resource_id, vr_path, source_file_type, file_name, file_type, distribution_info)
//                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
//                         params![
//                             profile_name,
//                             resource_id,
//                             path.format_to_string(),
//                             "standard",
//                             sf.file_name.clone(),
//                             serde_json::to_string(&sf.file_type).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
//                             serde_json::to_vec(&sf.distribution_info).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
//                         ],
//                     )?;
//                 }
//                 SourceFile::TLSNotarized(sf) => {
//                     let file_path = file_dir.join(sf.file_name.clone());
//                     std::fs::write(file_path, sf.file_content.clone())
//                         .map_err(|_| SqliteManagerError::FailedFetchingValue)?;

//                     // Store the source file metadata in the database
//                     tx.execute(
//                         "INSERT OR REPLACE INTO source_file_maps
//                             (profile_name, vector_resource_id, vr_path, source_file_type, file_name, file_type, distribution_info)
//                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
//                         params![
//                             profile_name,
//                             resource_id,
//                             path.format_to_string(),
//                             "tls_notarized",
//                             sf.file_name.clone(),
//                             serde_json::to_string(&sf.file_type).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
//                             serde_json::to_vec(&sf.distribution_info).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
//                         ],
//                     )?;
//                 }
//             };
//         }

//         tx.commit()?;
//         Ok(())
//     }

//     pub fn get_source_file_map(
//         &self,
//         resource_id: &str,
//         profile: &ShinkaiName,
//     ) -> Result<SourceFileMap, SqliteManagerError> {
//         let profile_name = profile
//             .get_profile_name_string()
//             .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;
//         let source_files_dir = Self::get_source_files_path().join(Self::get_root_directory_name(resource_id));

//         let conn = self.get_connection()?;
//         let mut stmt = conn.prepare(
//             "SELECT vr_path, source_file_type, file_name, file_type, distribution_info
//                 FROM source_file_maps WHERE profile_name = ?1 AND vector_resource_id = ?2",
//         )?;
//         let source_files_iter = stmt.query_map(params![profile_name, resource_id], |row| {
//             let vr_path: String = row.get(0)?;
//             let source_file_type: String = row.get(1)?;
//             let file_name: String = row.get(2)?;
//             let file_type: String = row.get(3)?;
//             let distribution_info: Option<Vec<u8>> = row.get(4)?;

//             let vr_path = ShinkaiPath::from_string(&vr_path)
//                 .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::VRError(e))))?;
//             let file_dir = vr_path.path_ids.join("/");
//             let file_path = source_files_dir.join(file_dir).join(&file_name);

//             let file_type = serde_json::from_str(&file_type).map_err(|e| {
//                 rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
//             })?;
//             let distribution_info = serde_json::from_slice(&distribution_info.unwrap_or_default()).map_err(|e| {
//                 rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
//             })?;

//             let source_file = match source_file_type.as_str() {
//                 "standard" => SourceFile::Standard(StandardSourceFile {
//                     file_name: file_name,
//                     file_content: std::fs::read(file_path).map_err(|_| {
//                         rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::FailedFetchingValue))
//                     })?,
//                     file_type,
//                     distribution_info,
//                 }),
//                 "tls_notarized" => SourceFile::TLSNotarized(TLSNotarizedSourceFile {
//                     file_name: file_name,
//                     file_content: std::fs::read(file_path).map_err(|_| {
//                         rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::FailedFetchingValue))
//                     })?,
//                     file_type,
//                     distribution_info,
//                     proof: TLSNotaryProof::new(),
//                 }),
//                 _ => {
//                     return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
//                         SqliteManagerError::SerializationError(format!(
//                             "Invalid source file type: {}",
//                             source_file_type
//                         )),
//                     )))
//                 }
//             };

//             Ok((vr_path, source_file))
//         })?;

//         let mut source_file_map = SourceFileMap::new(Default::default());
//         for source_file in source_files_iter {
//             let (vr_path, source_file) = source_file?;
//             source_file_map.add_source_file(vr_path, source_file);
//         }

//         Ok(source_file_map)
//     }

//     fn get_source_files_path() -> std::path::PathBuf {
//         match std::env::var("NODE_STORAGE_PATH").ok() {
//             Some(path) => std::path::PathBuf::from(path).join("files"),
//             None => std::path::PathBuf::from("files"),
//         }
//     }

//     fn get_root_directory_name(name: &str) -> String {
//         let sanitized_dir = name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
//         format!("source_{}", sanitized_dir)
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use shinkai_vector_resources::{
//         model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference},
//         source::{DocumentFileType, SourceFileType},
//     };
//     use std::path::PathBuf;
//     use tempfile::NamedTempFile;

//     fn setup_test_db() -> SqliteManager {
//         let temp_file = NamedTempFile::new().unwrap();
//         let db_path = PathBuf::from(temp_file.path());
//         let api_url = String::new();
//         let model_type =
//             EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

//         std::env::set_var("NODE_STORAGE_PATH", db_path.parent().unwrap());

//         SqliteManager::new(db_path, api_url, model_type).unwrap()
//     }

//     #[test]
//     fn test_save_source_file_map() {
//         let db = setup_test_db();
//         let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
//         let resource_id = "test_resource";
//         let mut source_file_map = SourceFileMap::new(Default::default());
//         let vr_path = VRPath::new();
//         let source_file = SourceFile::Standard(StandardSourceFile {
//             file_name: "test_file.txt".to_string(),
//             file_content: b"test_content".to_vec(),
//             file_type: SourceFileType::Document(DocumentFileType::Txt),
//             distribution_info: None,
//         });
//         source_file_map.add_source_file(vr_path, source_file);

//         db.save_source_file_map(&source_file_map, resource_id, &profile)
//             .unwrap();

//         let saved_source_file_map = db.get_source_file_map(resource_id, &profile).unwrap();
//         assert_eq!(source_file_map, saved_source_file_map);
//     }
// }
