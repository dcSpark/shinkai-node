use super::file_parser_types::TextGroup;
use super::local_parsing::LocalFileParser;
#[cfg(feature = "desktop-only")]
use super::unstructured_api::UnstructuredAPI;
use crate::data_tags::DataTag;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
use crate::source::DistributionInfo;
use crate::source::TextChunkingStrategy;
use crate::source::VRSourceReference;
use crate::vector_resource::{BaseVectorResource, DocumentVectorResource, VectorResourceCore};
#[cfg(feature = "desktop-only")]
use async_recursion::async_recursion;

pub struct ShinkaiFileParser;

impl ShinkaiFileParser {
    #[cfg(any(feature = "dynamic-pdf-parser", feature = "static-pdf-parser"))]
    pub async fn initialize_local_file_parser() -> Result<(), Box<dyn std::error::Error>> {
        use shinkai_pdf_parser::pdf_parser::PDFParser;

        PDFParser::check_and_download_dependencies().await
    }

    #[cfg(feature = "desktop-only")]
    /// Processes the input file into a BaseVectorResource.
    pub async fn process_file_into_resource(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        desc: Option<String>,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        distribution_info: DistributionInfo,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, VRError> {
        let cleaned_name = ShinkaiFileParser::clean_name(&file_name);
        let source = VRSourceReference::from_file(&file_name, TextChunkingStrategy::V1)?;
        let text_groups = Self::process_file_into_text_groups(
            file_buffer,
            file_name,
            max_node_text_size,
            source.clone(),
            unstructured_api,
        )
        .await?;

        ShinkaiFileParser::process_groups_into_resource(
            text_groups,
            generator,
            cleaned_name,
            desc,
            source,
            parsing_tags,
            max_node_text_size,
            distribution_info,
        )
        .await
    }

    #[cfg(feature = "desktop-only")]
    /// Processes the input file into a BaseVectorResource.
    pub fn process_file_into_resource_blocking(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        desc: Option<String>,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        distribution_info: DistributionInfo,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, VRError> {
        let cleaned_name = ShinkaiFileParser::clean_name(&file_name);
        let source = VRSourceReference::from_file(&file_name, TextChunkingStrategy::V1)?;
        let text_groups = ShinkaiFileParser::process_file_into_text_groups_blocking(
            file_buffer,
            file_name,
            max_node_text_size,
            source.clone(),
            unstructured_api,
        )?;

        // Here, we switch to the blocking variant of `process_groups_into_resource`.
        ShinkaiFileParser::process_groups_into_resource_blocking(
            text_groups,
            generator,
            cleaned_name,
            desc,
            source,
            parsing_tags,
            max_node_text_size,
            distribution_info,
        )
    }

    #[cfg(feature = "desktop-only")]
    /// Processes the input file into a list of `TextGroup` with no embedding generated yet.
    pub async fn process_file_into_text_groups(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<TextGroup>, VRError> {
        // If local processing is available, use it. Otherwise, use the unstructured API.
        let text_groups = if let Ok(groups) = LocalFileParser::process_file_into_grouped_text(
            file_buffer.clone(),
            file_name.clone(),
            max_node_text_size,
            source.clone(),
        ) {
            groups
        } else {
            unstructured_api
                .process_file_into_grouped_text(file_buffer, file_name.clone(), max_node_text_size)
                .await?
        };

        Ok(text_groups)
    }

    #[cfg(feature = "desktop-only")]
    /// Processes the input file into a list of `TextGroup` with no embedding generated yet.
    pub fn process_file_into_text_groups_blocking(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<TextGroup>, VRError> {
        // If local processing is available, use it. Otherwise, use the unstructured API.
        let text_groups = if let Ok(groups) = LocalFileParser::process_file_into_grouped_text(
            file_buffer.clone(),
            file_name.clone(),
            max_node_text_size,
            source.clone(),
        ) {
            groups
        } else {
            // Assuming `process_file_into_grouped_text_blocking` is a synchronous version of `process_file_into_grouped_text`
            unstructured_api.process_file_into_grouped_text_blocking(
                file_buffer,
                file_name.clone(),
                max_node_text_size,
            )?
        };

        Ok(text_groups)
    }

    #[cfg(feature = "desktop-only")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource
    pub async fn process_groups_into_resource(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, VRError> {
        Self::process_groups_into_resource_with_custom_collection(
            text_groups,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            max_node_text_size,
            ShinkaiFileParser::collect_texts_and_indices,
            distribution_info,
        )
        .await
    }

    #[cfg(feature = "desktop-only")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource.
    pub fn process_groups_into_resource_blocking(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, VRError> {
        Self::process_groups_into_resource_blocking_with_custom_collection(
            text_groups,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            max_node_text_size,
            ShinkaiFileParser::collect_texts_and_indices,
            distribution_info,
        )
    }

    #[cfg(feature = "desktop-only")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource.
    /// Allows specifying a custom collection function.
    pub async fn process_groups_into_resource_with_custom_collection(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, VRError> {
        let new_text_groups = ShinkaiFileParser::generate_text_group_embeddings(
            &text_groups,
            generator.box_clone(),
            31,
            max_node_text_size,
            collect_texts_and_indices,
        )
        .await?;

        let mut resource = ShinkaiFileParser::process_new_doc_resource_with_embeddings_already_generated(
            new_text_groups,
            &*generator,
            &name,
            desc,
            source,
            parsing_tags,
            None,
        )
        .await?;
        resource.as_trait_object_mut().set_distribution_info(distribution_info);
        Ok(resource)
    }

    #[cfg(feature = "desktop-only")]
    /// Processes an ordered list of `TextGroup`s into a
    /// a ready-to-go BaseVectorResource. Allows specifying a custom collection function.
    pub fn process_groups_into_resource_blocking_with_custom_collection(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, VRError> {
        // Group elements together before generating the doc
        let cloned_generator = generator.box_clone();

        // Use block_on to run the async-based batched embedding generation logic
        let new_text_groups = ShinkaiFileParser::generate_text_group_embeddings_blocking(
            &text_groups,
            cloned_generator,
            31,
            max_node_text_size,
            collect_texts_and_indices,
        )?;

        let mut resource = ShinkaiFileParser::process_new_doc_resource_blocking_with_embeddings_already_generated(
            new_text_groups,
            &*generator,
            &name,
            desc,
            source,
            parsing_tags,
            None,
        )?;

        resource.as_trait_object_mut().set_distribution_info(distribution_info);
        Ok(resource)
    }

    #[async_recursion]
    #[cfg(feature = "desktop-only")]
    /// Recursively processes all text groups & their sub groups into DocumentResources.
    /// This method assumes your text groups already have embeddings generated for them.
    async fn process_new_doc_resource_with_embeddings_already_generated(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let name = ShinkaiFileParser::clean_name(&name);
        let max_embedding_token_count = generator.model_type().max_input_token_count();
        let resource_desc = Self::_setup_resource_description(
            desc,
            &text_groups,
            max_embedding_token_count,
            max_embedding_token_count.checked_div(2).unwrap_or(100),
        );
        let mut doc = DocumentVectorResource::new_empty(&name, resource_desc.as_deref(), source.clone(), true);
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
                let new_doc = Self::process_new_doc_resource_with_embeddings_already_generated(
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

    #[cfg(feature = "desktop-only")]
    /// Recursively processes all text groups & their sub groups into DocumentResources.
    /// This method assumes your text groups already have embeddings generated for them.
    fn process_new_doc_resource_blocking_with_embeddings_already_generated(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let name = ShinkaiFileParser::clean_name(&name);
        let max_embedding_token_count = generator.model_type().max_input_token_count();
        let resource_desc = Self::_setup_resource_description(
            desc,
            &text_groups,
            max_embedding_token_count,
            max_embedding_token_count / 2,
        );
        let mut doc = DocumentVectorResource::new_empty(&name, resource_desc.as_deref(), source.clone(), true);
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
            let (_new_resource_id, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
            if has_sub_groups {
                let new_doc = Self::process_new_doc_resource_blocking_with_embeddings_already_generated(
                    grouped_text.sub_groups.clone(),
                    generator,
                    &new_name,
                    None,
                    source.clone(),
                    parsing_tags,
                    grouped_text.embedding.clone(),
                )?;
                let _ = doc.append_vector_resource_node_auto(new_doc, metadata);
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    let _ = doc.append_text_node(&grouped_text.text, metadata, embedding.clone(), parsing_tags);
                } else {
                    let embedding = generator.generate_embedding_default_blocking(&grouped_text.text)?;
                    let _ = doc.append_text_node(&grouped_text.text, metadata, embedding, parsing_tags);
                }
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }
}
