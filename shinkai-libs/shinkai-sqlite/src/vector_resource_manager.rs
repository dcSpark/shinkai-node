use std::collections::HashMap;

use bytemuck::cast_slice;
use rusqlite::params;
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
            conn.prepare("SELECT * FROM vector_resources WHERE vector_resource_id = ? AND profile_name = ?")?;
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
}
