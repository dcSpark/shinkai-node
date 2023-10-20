use super::unstructured_types::{GroupedText, UnstructuredElement};
use crate::base_vector_resources::BaseVectorResource;
use crate::data_tags::DataTag;
use crate::document_resource::DocumentVectorResource;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::VectorResource;
#[cfg(feature = "native-http")]
use async_recursion::async_recursion;
use blake3::Hasher;
use serde_json::Value as JsonValue;
#[cfg(feature = "native-http")]
use tokio::runtime::Runtime;

/// Struct which contains several methods related to parsing output from Unstructured
#[derive(Debug)]
pub struct UnstructuredParser;

impl UnstructuredParser {
    /// Parses the JSON Array response from Unstructured into a list of `UnstructuredElement`s
    pub fn parse_response_json(json: JsonValue) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
        if let JsonValue::Array(array) = json {
            let mut elements = Vec::new();
            for item in array {
                let element: UnstructuredElement = serde_json::from_value(item)
                    .map_err(|err| VectorResourceError::FailedParsingUnstructedAPIJSON(err.to_string()))?;
                elements.push(element);
            }
            Ok(elements)
        } else {
            Err(VectorResourceError::FailedParsingUnstructedAPIJSON(
                "Response is not an array at top level".to_string(),
            ))
        }
    }

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource
    pub async fn process_elements_into_resource(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: String,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        Self::process_elements_into_resource_with_custom_collection(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            resource_id,
            max_chunk_size,
            Self::collect_texts_and_indices,
        )
        .await
    }

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource.
    pub fn process_elements_into_resource_blocking(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: String,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        Self::process_elements_into_resource_blocking_with_custom_collection(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            resource_id,
            max_chunk_size,
            Self::collect_texts_and_indices,
        )
    }

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource. Allows specifying a custom collection function.
    pub async fn process_elements_into_resource_with_custom_collection(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: String,
        max_chunk_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Group elements together before generating the doc
        let text_groups = UnstructuredParser::hierarchical_group_elements_text(&elements, max_chunk_size);
        let cloned_generator = generator.box_clone();
        let task = tokio::spawn(async move {
            Self::generate_text_group_embeddings(
                &text_groups,
                cloned_generator,
                31,
                max_chunk_size,
                collect_texts_and_indices,
            )
            .await
        });
        let new_text_groups = task.await??;

        Self::process_new_doc_resource(
            new_text_groups,
            &*generator,
            &name,
            desc,
            source,
            parsing_tags,
            &resource_id,
            None,
        )
        .await
    }

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource. Allows specifying a custom collection function.
    pub fn process_elements_into_resource_blocking_with_custom_collection(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: String,
        max_chunk_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Create a new Tokio runtime
        let rt = Runtime::new().unwrap();

        // Group elements together before generating the doc
        let text_groups = UnstructuredParser::hierarchical_group_elements_text(&elements, max_chunk_size);
        let cloned_generator = generator.box_clone();

        // Use block_on to run the async-based batched embedding generation logic
        let new_text_groups = rt.block_on(async {
            Self::generate_text_group_embeddings(
                &text_groups,
                cloned_generator,
                31,
                max_chunk_size,
                collect_texts_and_indices,
            )
            .await
        })?;

        Self::process_new_doc_resource_blocking(
            new_text_groups,
            &*generator,
            &name,
            desc,
            source,
            parsing_tags,
            &resource_id,
            None,
        )
    }

    #[async_recursion]
    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources
    async fn process_new_doc_resource(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        let resource_desc = Self::setup_resource_description(desc, &text_groups);
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc.as_deref(), source.clone(), &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Sets a Resource Embedding if none provided. Primarily only used at the root level as the rest should already have them.
        match resource_embedding {
            Some(embedding) => doc.set_resource_embedding(embedding),
            None => {
                println!("Generating embedding for resource: {:?}", &name);
                let keywords = UnstructuredParser::extract_keywords(&text_groups, 50);
                doc.update_resource_embedding(generator, keywords).await?;
            }
        }

        // Add each text group as either Vector Resource DataChunks,
        // or data-holding DataChunks depending on if each has any sub-groups
        for grouped_text in &text_groups {
            let (new_resource_id, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
            if has_sub_groups {
                let new_doc = Self::process_new_doc_resource(
                    grouped_text.sub_groups.clone(),
                    generator,
                    &new_name,
                    None,
                    source.clone(),
                    parsing_tags,
                    &new_resource_id,
                    grouped_text.embedding.clone(),
                )
                .await?;
                doc.append_vector_resource(new_doc, metadata);
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    doc.append_data(&grouped_text.text, metadata, &embedding, parsing_tags);
                } else {
                    println!("Generating embedding for: {:?}", &grouped_text.text);
                    let embedding = generator.generate_embedding_default(&grouped_text.text).await?;
                    doc.append_data(&grouped_text.text, metadata, &embedding, parsing_tags);
                }
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }

    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources
    fn process_new_doc_resource_blocking(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        let resource_desc = Self::setup_resource_description(desc, &text_groups);
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc.as_deref(), source.clone(), &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Sets a Resource Embedding if none provided. Primarily only used at the root level as the rest should already have them.
        match resource_embedding {
            Some(embedding) => doc.set_resource_embedding(embedding),
            None => {
                println!("Generating embedding for resource: {:?}", &name);
                let keywords = UnstructuredParser::extract_keywords(&text_groups, 50);
                doc.update_resource_embedding_blocking(generator, keywords)?;
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
                    &new_resource_id,
                    grouped_text.embedding.clone(),
                )?;
                doc.append_vector_resource(new_doc, metadata);
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    doc.append_data(&grouped_text.text, metadata, &embedding, parsing_tags);
                } else {
                    println!("Generating embedding for: {:?}", &grouped_text.text);
                    let embedding = generator.generate_embedding_default_blocking(&grouped_text.text)?;
                    doc.append_data(&grouped_text.text, metadata, &embedding, parsing_tags);
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
