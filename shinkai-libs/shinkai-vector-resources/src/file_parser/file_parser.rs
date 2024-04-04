use super::file_parser_types::GroupedText;
use crate::data_tags::DataTag;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
use crate::source::DistributionInfo;
use crate::source::VRSourceReference;
use crate::vector_resource::{BaseVectorResource, DocumentVectorResource, VectorResource, VectorResourceCore};
#[cfg(feature = "native-http")]
use async_recursion::async_recursion;
use blake3::Hasher;
use serde_json::Value as JsonValue;

pub struct ShinkaiFileParser;

impl ShinkaiFileParser {
    #[async_recursion]
    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources
    pub async fn process_new_doc_resource(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let resource_desc = Self::setup_resource_description(desc, &text_groups);
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc.as_deref(), source.clone(), true);
        doc.set_embedding_model_used(generator.model_type());

        // Sets the keywords
        let keywords = Self::extract_keywords(&text_groups, 25);
        doc.keywords_mut().set_keywords(keywords.clone());
        doc.keywords_mut().update_keywords_embedding(generator).await?;
        // Sets a Resource Embedding if none provided. Primarily only used at the root level as the rest should already have them.
        match resource_embedding {
            Some(embedding) => doc.set_resource_embedding(embedding),
            None => {
                doc.update_resource_embedding(generator, None).await?;
            }
        }

        // Add each text group as either Vector Resource Nodes,
        // or data-holding Nodes depending on if each has any sub-groups
        for grouped_text in &text_groups {
            let (_, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
            if has_sub_groups {
                let new_doc = Self::process_new_doc_resource(
                    grouped_text.sub_groups.clone(),
                    generator,
                    &new_name,
                    None,
                    source.clone(),
                    parsing_tags,
                    grouped_text.embedding.clone(),
                )
                .await?;
                doc.append_vector_resource_node_auto(new_doc, metadata)?;
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    doc.append_text_node(&grouped_text.text, metadata, embedding.clone(), parsing_tags)?;
                } else {
                    let embedding = generator.generate_embedding_default(&grouped_text.text).await?;
                    doc.append_text_node(&grouped_text.text, metadata, embedding, parsing_tags)?;
                }
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }

    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources
    pub fn process_new_doc_resource_blocking(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let resource_desc = Self::setup_resource_description(desc, &text_groups);
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc.as_deref(), source.clone(), true);
        doc.set_embedding_model_used(generator.model_type());

        // Sets the keywords and generates a keyword embedding
        let keywords = Self::extract_keywords(&text_groups, 25);
        doc.keywords_mut().set_keywords(keywords.clone());
        doc.keywords_mut().update_keywords_embedding_blocking(generator)?;
        // Sets a Resource Embedding if none provided. Primarily only used at the root level as the rest should already have them.
        match resource_embedding {
            Some(embedding) => doc.set_resource_embedding(embedding),
            None => {
                doc.update_resource_embedding_blocking(generator, None)?;
            }
        }

        for grouped_text in &text_groups {
            let (new_resource_id, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
            if has_sub_groups {
                let new_doc = Self::process_new_doc_resource_blocking(
                    grouped_text.sub_groups.clone(),
                    generator,
                    &new_name,
                    None,
                    source.clone(),
                    parsing_tags,
                    grouped_text.embedding.clone(),
                )?;
                doc.append_vector_resource_node_auto(new_doc, metadata);
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    doc.append_text_node(&grouped_text.text, metadata, embedding.clone(), parsing_tags);
                } else {
                    let embedding = generator.generate_embedding_default_blocking(&grouped_text.text)?;
                    doc.append_text_node(&grouped_text.text, metadata, embedding, parsing_tags);
                }
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }

    /// Helper method for setting a description if none provided for process_new_doc_resource
    fn setup_resource_description(desc: Option<String>, text_groups: &Vec<GroupedText>) -> Option<String> {
        if let Some(description) = desc {
            Some(description.to_string())
        } else if !text_groups.is_empty() {
            Some(text_groups[0].text.to_string())
        } else {
            None
        }
    }

    /// Generates a Blake3 hash of the data in the buffer
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }
}
