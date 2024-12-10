use std::collections::HashMap;

use bytemuck::cast_slice;
use rusqlite::{params, Transaction};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    data_tags::DataTagIndex,
    embeddings::Embedding,
    model_type::EmbeddingModelType,
    resource_errors::VRError,
    source::{DistributionInfo, VRSourceReference},
    vector_resource::{
        BaseVectorResource, DocumentVectorResource, MapVectorResource, Node, NodeContent, VRBaseType, VRHeader,
        VRKeywords,
    },
};

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn save_resource(&self, resource: &BaseVectorResource, profile_name: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        self.save_resource_tx(&tx, resource, profile_name)?;

        tx.commit()?;
        Ok(())
    }

    fn save_resource_tx(
        &self,
        tx: &Transaction,
        resource: &BaseVectorResource,
        profile_name: &str,
    ) -> Result<(), SqliteManagerError> {
        let vector_resource_id = &resource.as_trait_object().reference_string();
        let resource = resource.as_trait_object();

        // Insert into the vector_resources table
        tx.execute(
            "INSERT INTO vector_resources (
                profile_name,
                vector_resource_id,
                name,
                description,
                source,
                resource_id,
                resource_base_type,
                embedding_model_used_string,
                node_count,
                data_tag_index,
                created_datetime,
                last_written_datetime,
                metadata_index,
                merkle_root,
                keywords,
                distribution_info
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                profile_name,
                vector_resource_id,
                resource.name(),
                resource.description(),
                serde_json::to_string(&resource.source())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                resource.resource_id(),
                serde_json::to_string(&resource.resource_base_type())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                resource.embedding_model_used_string(),
                resource.node_count(),
                serde_json::to_vec(&resource.data_tag_index())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                resource.created_datetime().to_rfc3339(),
                resource.last_written_datetime().to_rfc3339(),
                serde_json::to_vec(&resource.metadata_index())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                resource.get_merkle_root().ok(),
                serde_json::to_vec(&resource.keywords())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                serde_json::to_vec(&resource.distribution_info())
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
            ],
        )?;

        // Insert resource_embedding into the vector_resource_embeddings table
        let resource_embedding = resource.resource_embedding();
        tx.execute(
            "INSERT INTO vector_resource_embeddings (
                profile_name,
                vector_resource_id,
                id,
                embedding,
                is_resource_embedding
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile_name,
                vector_resource_id,
                resource_embedding.id,
                cast_slice(&resource_embedding.vector),
                true,
            ],
        )?;

        // Insert embeddings into the vector_resource_embeddings table
        for embedding in resource.get_root_embeddings() {
            tx.execute(
                "INSERT INTO vector_resource_embeddings (
                    profile_name,
                    vector_resource_id,
                    id,
                    embedding,
                    is_resource_embedding
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    profile_name,
                    vector_resource_id,
                    embedding.id,
                    cast_slice(&embedding.vector),
                    false,
                ],
            )?;
        }

        // Insert nodes into the vector_resource_nodes table
        for node in resource.get_root_nodes() {
            let content_type = match node.content {
                NodeContent::ExternalContent(_) => "external",
                NodeContent::Resource(_) => "resource",
                NodeContent::VRHeader(_) => "header",
                NodeContent::Text(_) => "text",
            };

            let content_value = match &node.content {
                NodeContent::ExternalContent(external_content) => serde_json::to_string(&external_content)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                NodeContent::Resource(resource) => resource.as_trait_object().reference_string(),
                NodeContent::VRHeader(header) => header.reference_string(),
                NodeContent::Text(text) => text.to_string(),
            };

            // Save resource or VRHeader
            if let NodeContent::Resource(resource) = &node.content {
                self.save_resource_tx(tx, resource, profile_name)?;
            } else if let NodeContent::VRHeader(header) = &node.content {
                self.save_vr_header_tx(tx, header, profile_name)?;
            }

            tx.execute(
                "INSERT INTO vector_resource_nodes (
                    profile_name,
                    vector_resource_id,
                    id,
                    content_type,
                    content_value,
                    metadata,
                    data_tag_names,
                    last_written_datetime,
                    merkle_hash
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    profile_name,
                    vector_resource_id,
                    node.id,
                    content_type,
                    content_value,
                    serde_json::to_string(&node.metadata)
                        .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                    serde_json::to_string(&node.data_tag_names)
                        .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                    node.last_written_datetime.to_rfc3339(),
                    node.merkle_hash,
                ],
            )?;
        }

        Ok(())
    }

    pub fn delete_resource(&self, reference_string: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM vector_resources WHERE vector_resource_id = ?",
            params![reference_string],
        )?;
        tx.execute(
            "DELETE FROM vector_resource_embeddings WHERE vector_resource_id = ?",
            params![reference_string],
        )?;
        tx.execute(
            "DELETE FROM vector_resource_nodes WHERE vector_resource_id = ?",
            params![reference_string],
        )?;
        tx.execute(
            "DELETE FROM vector_resource_headers WHERE vector_resource_id = ?",
            params![reference_string],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_resource(
        &self,
        vector_resource_id: &str,
        profile: &ShinkaiName,
    ) -> Result<BaseVectorResource, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        // Fetch the vector resource
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT * FROM vector_resources WHERE vector_resource_id = ?1 AND profile_name = ?2")?;
        let resource = stmt.query_row(params![vector_resource_id, profile_name], |row| {
            let name: String = row.get(2)?;
            let description: Option<String> = row.get(3)?;
            let source: String = row.get(4)?;
            let resource_id: String = row.get(5)?;
            let resource_base_type: String = row.get(6)?;
            let embedding_model_used_string: String = row.get(7)?;
            let node_count: u64 = row.get(8)?;
            let data_tag_index: Vec<u8> = row.get(9)?;
            let created_datetime: String = row.get(10)?;
            let last_written_datetime: String = row.get(11)?;
            let metadata_index: Vec<u8> = row.get(12)?;
            let merkle_root: Option<String> = row.get(13)?;
            let keywords: Vec<u8> = row.get(14)?;
            let distribution_info: Vec<u8> = row.get(15)?;

            let source: VRSourceReference = serde_json::from_str(&source).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let resource_base_type: VRBaseType = serde_json::from_str(&resource_base_type).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let data_tag_index: DataTagIndex = serde_json::from_slice(&data_tag_index).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let created_datetime = created_datetime.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(e.to_string())))
            })?;
            let last_written_datetime =
                last_written_datetime
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?;
            let metadata_index = serde_json::from_slice(&metadata_index).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let keywords: VRKeywords = serde_json::from_slice(&keywords).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let distribution_info: DistributionInfo = serde_json::from_slice(&distribution_info).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            let resource_embedding = self
                .get_embeddings(vector_resource_id, profile, true)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                .pop()
                .ok_or(SqliteManagerError::MissingValue("resource_embedding".to_string()))
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            match resource_base_type {
                VRBaseType::Document => {
                    let document_resource = BaseVectorResource::Document(DocumentVectorResource {
                        resource_base_type,
                        embeddings: vec![],
                        nodes: vec![],
                        name,
                        description,
                        source,
                        resource_id,
                        embedding_model_used_string,
                        node_count,
                        data_tag_index,
                        created_datetime,
                        last_written_datetime,
                        metadata_index,
                        merkle_root,
                        keywords,
                        distribution_info,
                        resource_embedding,
                    });

                    Ok(document_resource)
                }
                VRBaseType::Map => {
                    let map_resource = BaseVectorResource::Map(MapVectorResource {
                        resource_base_type,
                        embeddings: HashMap::new(),
                        nodes: HashMap::new(),
                        name,
                        description,
                        source,
                        resource_id,
                        embedding_model_used_string,
                        node_count,
                        data_tag_index,
                        created_datetime,
                        last_written_datetime,
                        metadata_index,
                        merkle_root,
                        keywords,
                        distribution_info,
                        resource_embedding,
                    });

                    Ok(map_resource)
                }
                _ => Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    SqliteManagerError::VRError(VRError::InvalidVRBaseType),
                ))),
            }
        });

        let mut resource = match resource {
            Ok(resource) => Ok(resource),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(SqliteManagerError::DataNotFound),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }?;

        // Fetch the embeddings
        let embeddings = self.get_embeddings(vector_resource_id, profile, false)?;

        // Fetch the nodes
        let nodes = self.get_nodes(vector_resource_id, profile)?;

        match resource {
            BaseVectorResource::Document(ref mut document_resource) => {
                document_resource.embeddings = embeddings;
                document_resource.nodes = nodes;
            }
            BaseVectorResource::Map(ref mut map_resource) => {
                map_resource.embeddings = embeddings.into_iter().map(|e| (e.id.clone(), e)).collect();
                map_resource.nodes = nodes.into_iter().map(|n| (n.id.clone(), n)).collect();
            }
        }

        Ok(resource)
    }

    fn get_embeddings(
        &self,
        vector_resource_id: &str,
        profile: &ShinkaiName,
        is_resource_embedding: bool,
    ) -> Result<Vec<Embedding>, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, embedding FROM vector_resource_embeddings WHERE vector_resource_id = ? AND profile_name = ? AND is_resource_embedding = ?")?;
        let embeddings = stmt.query_map(
            params![vector_resource_id, profile_name, is_resource_embedding],
            |row| {
                let id: String = row.get(0)?;
                let embedding_bytes: Vec<u8> = row.get(1)?;
                let embedding: &[f32] = cast_slice(&embedding_bytes);

                Ok(Embedding {
                    id,
                    vector: embedding.to_vec(),
                })
            },
        )?;

        let embeddings = embeddings.collect::<Result<Vec<Embedding>, _>>()?;

        Ok(embeddings)
    }

    fn get_nodes(&self, vector_resource_id: &str, profile: &ShinkaiName) -> Result<Vec<Node>, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT * FROM vector_resource_nodes WHERE vector_resource_id = ? AND profile_name = ?")?;
        let nodes = stmt.query_map(params![vector_resource_id, profile_name], |row| {
            let id: String = row.get(2)?;
            let content_type: String = row.get(3)?;
            let content_value: String = row.get(4)?;
            let metadata: Option<String> = row.get(5)?;
            let data_tag_names: String = row.get(6)?;
            let last_written_datetime: String = row.get(7)?;
            let merkle_hash: Option<String> = row.get(8)?;

            let metadata: Option<HashMap<String, String>> = match metadata {
                Some(metadata) => serde_json::from_str(&metadata).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                None => None,
            };

            let content = match content_type.as_str() {
                "external" => NodeContent::ExternalContent(serde_json::from_str(&content_value).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?),
                "resource" => NodeContent::Resource(
                    self.get_resource(&content_value, profile)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                ),
                "header" => NodeContent::VRHeader(
                    self.get_vr_header(&content_value, profile)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                ),
                "text" | _ => NodeContent::Text(content_value),
            };

            Ok(Node {
                id,
                content,
                metadata,
                data_tag_names: serde_json::from_str(&data_tag_names).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                last_written_datetime: last_written_datetime
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                merkle_hash,
            })
        })?;

        let nodes = nodes.collect::<Result<Vec<Node>, _>>()?;

        Ok(nodes)
    }

    fn get_vr_header(&self, vector_resource_id: &str, profile: &ShinkaiName) -> Result<VRHeader, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT * FROM vector_resource_headers WHERE vector_resource_id = ? AND profile_name = ?")?;
        let vr_header = stmt.query_row(params![vector_resource_id, profile_name], |row| {
            let resource_name: String = row.get(2)?;
            let resource_id: String = row.get(3)?;
            let resource_base_type: String = row.get(4)?;
            let resource_source: String = row.get(5)?;
            let resource_created_datetime: String = row.get(6)?;
            let resource_last_written_datetime: String = row.get(7)?;
            let resource_embedding_model_used: String = row.get(8)?;
            let resource_merkle_root: Option<String> = row.get(9)?;
            let resource_keywords: Vec<u8> = row.get(10)?;
            let resource_distribution_info: Vec<u8> = row.get(11)?;
            let data_tag_names: String = row.get(12)?;
            let metadata_index_keys: String = row.get(13)?;

            let resource_base_type: VRBaseType = serde_json::from_str(&resource_base_type).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let resource_source: VRSourceReference = serde_json::from_str(&resource_source).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let resource_embedding_model_used: EmbeddingModelType =
                serde_json::from_str(&resource_embedding_model_used).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let resource_distribution_info: DistributionInfo = serde_json::from_slice(&resource_distribution_info)
                .map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
            let data_tag_names: Vec<String> = serde_json::from_str(&data_tag_names).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let metadata_index_keys: Vec<String> = serde_json::from_str(&metadata_index_keys).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            let resource_embedding = self
                .get_embeddings(vector_resource_id, profile, true)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                .pop();

            Ok(VRHeader {
                resource_name,
                resource_id,
                resource_base_type,
                resource_source,
                resource_created_datetime: resource_created_datetime
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                resource_last_written_datetime: resource_last_written_datetime
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                resource_embedding_model_used,
                resource_merkle_root,
                resource_keywords: serde_json::from_slice(&resource_keywords).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                resource_distribution_info,
                data_tag_names,
                metadata_index_keys,
                resource_embedding,
            })
        });

        let vr_header = match vr_header {
            Ok(vr_header) => Ok(vr_header),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(SqliteManagerError::DataNotFound),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }?;

        Ok(vr_header)
    }

    fn save_vr_header_tx(
        &self,
        tx: &Transaction,
        vr_header: &VRHeader,
        profile_name: &str,
    ) -> Result<(), SqliteManagerError> {
        tx.execute(
            "INSERT INTO vector_resource_headers (
                profile_name,
                vector_resource_id,
                resource_name,
                resource_id,
                resource_base_type,
                resource_source,
                resource_created_datetime,
                resource_last_written_datetime,
                resource_embedding_model_used,
                resource_merkle_root,
                resource_keywords,
                resource_distribution_info,
                data_tag_names,
                metadata_index_keys
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                profile_name,
                vr_header.resource_id,
                vr_header.resource_name,
                vr_header.resource_id,
                serde_json::to_string(&vr_header.resource_base_type)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                serde_json::to_string(&vr_header.resource_source)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                vr_header.resource_created_datetime.to_rfc3339(),
                vr_header.resource_last_written_datetime.to_rfc3339(),
                serde_json::to_string(&vr_header.resource_embedding_model_used)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                vr_header.resource_merkle_root,
                serde_json::to_vec(&vr_header.resource_keywords)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                serde_json::to_vec(&vr_header.resource_distribution_info)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                serde_json::to_string(&vr_header.data_tag_names)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
                serde_json::to_string(&vr_header.metadata_index_keys)
                    .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?,
            ],
        )?;

        // Insert resource_embedding into the vector_resource_embeddings table
        if let Some(resource_embedding) = &vr_header.resource_embedding {
            tx.execute(
                "INSERT INTO vector_resource_embeddings (
                    profile_name,
                    vector_resource_id,
                    id,
                    embedding,
                    is_resource_embedding
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    profile_name,
                    vr_header.reference_string(),
                    resource_embedding.id,
                    cast_slice(&resource_embedding.vector),
                    true,
                ],
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::{
        embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
        model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference},
        vector_resource::VectorResourceCore,
    };
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_document_vector_resources() {
        let manager = setup_test_db();

        let generator = RemoteEmbeddingGenerator::new_default();
        let mut doc = DocumentVectorResource::new_empty(
            "Test VR",
            Some("Test VR Description"),
            VRSourceReference::new_uri_ref("https://example.com"),
            true,
        );

        doc.set_embedding_model_used(generator.model_type());
        doc.update_resource_embedding(&generator, Some(vec!["test".to_string(), "document".to_string()]))
            .await
            .unwrap();

        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let vr = BaseVectorResource::Document(doc.clone());

        manager
            .save_resource(&vr, &profile.get_profile_name_string().unwrap())
            .unwrap();

        let vr2 = manager.get_resource(&doc.reference_string(), &profile).unwrap();

        assert_eq!(vr, vr2);
    }

    #[tokio::test]
    async fn test_nested_vr_with_nodes() {
        let manager = setup_test_db();

        let generator = RemoteEmbeddingGenerator::new_default();
        let mut map_resource = MapVectorResource::new_empty(
            "Tech Facts",
            Some("A collection of facts about technology"),
            VRSourceReference::new_uri_ref("veryrealtechfacts.com"),
            true,
        );

        map_resource.set_embedding_model_used(generator.model_type()); // Not required, but good practice
        map_resource
            .update_resource_embedding(&generator, Some(vec!["technology".to_string(), "phones".to_string()]))
            .await
            .unwrap();

        let mut doc_resource = DocumentVectorResource::new_empty(
            "Test VR",
            Some("Test VR Description"),
            VRSourceReference::new_uri_ref("https://example.com"),
            true,
        );

        doc_resource.set_embedding_model_used(generator.model_type());
        doc_resource
            .update_resource_embedding(&generator, Some(vec!["test".to_string(), "document".to_string()]))
            .await
            .unwrap();

        let doc_name = doc_resource.name.clone();
        let node = Node::new_vector_resource(doc_name.clone(), &BaseVectorResource::Document(doc_resource), None);
        let embedding = generator.generate_embedding_default("test node").await.unwrap();
        map_resource
            .insert_node_dt_specified(doc_name, node, embedding.clone(), None, true)
            .unwrap();

        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let vr = BaseVectorResource::Map(map_resource.clone());

        manager
            .save_resource(&vr, &profile.get_profile_name_string().unwrap())
            .unwrap();

        let vr2 = manager
            .get_resource(&map_resource.reference_string(), &profile)
            .unwrap();

        assert_eq!(vr, vr2);
    }

    #[tokio::test]
    async fn test_delete_resource() {
        let manager = setup_test_db();

        let generator = RemoteEmbeddingGenerator::new_default();
        let mut doc = DocumentVectorResource::new_empty(
            "Test VR",
            Some("Test VR Description"),
            VRSourceReference::new_uri_ref("https://example.com"),
            true,
        );

        doc.set_embedding_model_used(generator.model_type());
        doc.update_resource_embedding(&generator, Some(vec!["test".to_string(), "document".to_string()]))
            .await
            .unwrap();

        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let vr = BaseVectorResource::Document(doc.clone());

        manager
            .save_resource(&vr, &profile.get_profile_name_string().unwrap())
            .unwrap();

        manager.delete_resource(&doc.reference_string()).unwrap();

        let vr2 = manager.get_resource(&doc.reference_string(), &profile);

        assert!(vr2.is_err());
    }
}
