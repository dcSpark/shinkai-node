use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use std::{future::Future, pin::Pin};

use crate::shinkai_fs_error::ShinkaiFsError;

use super::{file_parser_types::TextGroup, utils::TextChunkingStrategy};
use super::local_parsing::LocalFileParser;

pub struct ShinkaiFileParser;

impl ShinkaiFileParser {
    /// Optionally, if you need some global initialization for OCR, etc.
    pub async fn initialize_local_file_parser() -> Result<(), Box<dyn std::error::Error>> {
        use shinkai_ocr::image_parser::ImageParser;
        ImageParser::check_and_download_dependencies().await
    }

    /// Processes the input file into a BaseVectorResource, auto-detecting extension
    /// and using local parsing. Then runs embedding logic.
    pub async fn process_file_into_resource(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        desc: Option<String>,
        parsing_tags: &Vec<String>,
        max_node_text_size: u64,
    ) -> Result<BaseVectorResource, ShinkaiFsError> {
        let cleaned_name = ShinkaiFileParser::clean_name(&file_name);

        // 1) Parse into text groups
        let text_groups = Self::process_file_into_text_groups(file_buffer, file_name, max_node_text_size).await?;

        // 2) Turn those text groups into a resource
        Self::process_groups_into_resource(
            text_groups,
            generator,
            cleaned_name,
            desc,
            parsing_tags,
            max_node_text_size,
        )
        .await
    }

    /// Processes the input file into a list of `TextGroup` with no embedding generated yet,
    /// auto-detecting the file type by extension.
    pub async fn process_file_into_text_groups(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        // The new LocalFileParser method automatically detects extension from `file_name`
        LocalFileParser::process_file_into_grouped_text(file_buffer, file_name, max_node_text_size)
    }

    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource
    pub async fn process_groups_into_resource(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        parsing_tags: &Vec<String>,
        max_node_text_size: u64,
    ) -> Result<BaseVectorResource, ShinkaiFsError> {
        // We keep the same pattern as before but remove references to `source`
        Self::process_groups_into_resource_with_custom_collection(
            text_groups,
            generator,
            name,
            desc,
            parsing_tags,
            max_node_text_size,
            ShinkaiFileParser::collect_texts_and_indices,
        )
        .await
    }

    /// Same as above, but allows a custom function for collecting text/index pairs
    pub async fn process_groups_into_resource_with_custom_collection(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        parsing_tags: &Vec<String>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
    ) -> Result<BaseVectorResource, ShinkaiFsError> {
        // Generate embeddings for all text groups
        let new_text_groups = ShinkaiFileParser::generate_text_group_embeddings(
            text_groups,
            generator.box_clone(),
            31,
            max_node_text_size,
            collect_texts_and_indices,
        )
        .await?;

        // Build a resource from those text groups
        let mut resource = ShinkaiFileParser::process_new_doc_resource_with_embeddings_already_generated(
            new_text_groups,
            &*generator,
            &name,
            desc,
            parsing_tags,
            None,
        )
        .await?;

        // In your code, presumably you have something like `distribution_info` you want to set:
        // resource.as_trait_object_mut().set_distribution_info(distribution_info);

        Ok(resource)
    }

    /// Blocking version
    pub fn process_groups_into_resource_blocking_with_custom_collection(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        parsing_tags: &Vec<String>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, ShinkaiFsError> {
        let cloned_generator = generator.box_clone();

        // Generate embeddings (blocking)
        let new_text_groups = ShinkaiFileParser::generate_text_group_embeddings_blocking(
            &text_groups,
            cloned_generator,
            31,
            max_node_text_size,
            collect_texts_and_indices,
        )?;

        // Build the resource
        let mut resource = ShinkaiFileParser::process_new_doc_resource_blocking_with_embeddings_already_generated(
            new_text_groups,
            &*generator,
            &name,
            desc,
            parsing_tags,
            None,
        )?;

        resource.as_trait_object_mut().set_distribution_info(distribution_info);
        Ok(resource)
    }

    /// Async: builds a DocumentVectorResource from text groups that already have embeddings
    fn process_new_doc_resource_with_embeddings_already_generated<'a>(
        text_groups: Vec<TextGroup>,
        generator: &'a dyn EmbeddingGenerator,
        name: &'a str,
        desc: Option<String>,
        parsing_tags: &'a Vec<String>,
        resource_embedding: Option<Embedding>,
    ) -> Pin<Box<dyn Future<Output = Result<BaseVectorResource, ShinkaiFsError>> + Send + 'a>> {
        Box::pin(async move {
            let name = ShinkaiFileParser::clean_name(name);
            let max_embedding_token_count = generator.model_type().max_input_token_count();
            let resource_desc = Self::_setup_resource_description(
                desc,
                &text_groups,
                max_embedding_token_count,
                max_embedding_token_count.checked_div(2).unwrap_or(100),
            );

            let mut doc = DocumentVectorResource::new_empty(&name, resource_desc.as_deref(), true);
            doc.set_embedding_model_used(generator.model_type());

            // Set keywords
            let keywords = Self::extract_keywords(&text_groups, 25);
            doc.keywords_mut().set_keywords(keywords.clone());
            doc.keywords_mut().update_keywords_embedding(generator).await?;

            // Possibly set the root resource embedding
            match resource_embedding {
                Some(embedding) => doc.set_resource_embedding(embedding),
                None => {
                    doc.update_resource_embedding(generator, None).await?;
                }
            }

            // Recursively add each text group
            for grouped_text in &text_groups {
                let (_, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
                if has_sub_groups {
                    let new_doc = Self::process_new_doc_resource_with_embeddings_already_generated(
                        grouped_text.sub_groups.clone(),
                        generator,
                        &new_name,
                        None,
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
        })
    }

    /// Blocking: builds a DocumentVectorResource from text groups that already have embeddings
    fn process_new_doc_resource_blocking_with_embeddings_already_generated(
        text_groups: Vec<TextGroup>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        parsing_tags: &Vec<String>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, ShinkaiFsError> {
        let name = ShinkaiFileParser::clean_name(name);
        let max_embedding_token_count = generator.model_type().max_input_token_count();
        let resource_desc = Self::_setup_resource_description(
            desc,
            &text_groups,
            max_embedding_token_count,
            max_embedding_token_count / 2,
        );
        let mut doc = DocumentVectorResource::new_empty(&name, resource_desc.as_deref(), true);
        doc.set_embedding_model_used(generator.model_type());

        // keywords
        let keywords = Self::extract_keywords(&text_groups, 25);
        doc.keywords_mut().set_keywords(keywords.clone());
        doc.keywords_mut().update_keywords_embedding_blocking(generator)?;

        // Possibly set the resource embedding
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
                    parsing_tags,
                    grouped_text.embedding.clone(),
                )?;
                doc.append_vector_resource_node_auto(new_doc, metadata)?;
            } else {
                if grouped_text.text.len() <= 2 {
                    continue;
                }
                if let Some(embedding) = &grouped_text.embedding {
                    doc.append_text_node(&grouped_text.text, metadata, embedding.clone(), parsing_tags)?;
                } else {
                    let embedding = generator.generate_embedding_default_blocking(&grouped_text.text)?;
                    doc.append_text_node(&grouped_text.text, metadata, embedding, parsing_tags)?;
                }
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }
}
