use super::file_parser_types::GroupedText;
use super::unstructured_api::UnstructuredAPI;
use crate::data_tags::DataTag;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
use crate::source::DistributionInfo;
use crate::source::TextChunkingStrategy;
use crate::source::VRSourceReference;
use crate::vector_resource::SourceFileReference;
use crate::vector_resource::SourceFileType;
use crate::vector_resource::SourceReference;
use crate::vector_resource::{BaseVectorResource, DocumentVectorResource, VectorResource, VectorResourceCore};
#[cfg(feature = "native-http")]
use async_recursion::async_recursion;
use blake3::Hasher;
use futures::stream::SelectNextSome;
use serde_json::Value as JsonValue;

pub struct ShinkaiFileParser;

impl ShinkaiFileParser {
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

    /// Processes the input file into a list of `GroupedText` with no embedding generated yet.
    pub async fn process_file_into_text_groups(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<GroupedText>, VRError> {
        let mut text_groups = vec![];

        // If local processing is available, use it. Otherwise, use the unstructured API.
        if let Ok(groups) = Self::local_process_file_into_grouped_text(
            file_buffer.clone(),
            file_name.clone(),
            max_node_text_size,
            source.clone(),
        ) {
            text_groups = groups;
        } else {
            text_groups = unstructured_api
                .process_file_into_grouped_text(file_buffer, file_name.clone(), max_node_text_size)
                .await?;
        }
        Ok(text_groups)
    }

    /// Processes the input file into a list of `GroupedText` with no embedding generated yet.
    pub fn process_file_into_text_groups_blocking(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<GroupedText>, VRError> {
        let mut text_groups = vec![];

        // If local processing is available, use it. Otherwise, use the unstructured API.
        if let Ok(groups) = Self::local_process_file_into_grouped_text(
            file_buffer.clone(),
            file_name.clone(),
            max_node_text_size,
            source.clone(),
        ) {
            text_groups = groups;
        } else {
            // Assuming `process_file_into_grouped_text_blocking` is a synchronous version of `process_file_into_grouped_text`
            text_groups = unstructured_api.process_file_into_grouped_text_blocking(
                file_buffer,
                file_name.clone(),
                max_node_text_size,
            )?;
        }
        Ok(text_groups)
    }

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource
    pub async fn process_groups_into_resource(
        text_groups: Vec<GroupedText>,
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

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource.
    pub fn process_groups_into_resource_blocking(
        text_groups: Vec<GroupedText>,
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

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `TextGroup`s into a ready-to-go BaseVectorResource.
    /// Allows specifying a custom collection function.
    pub async fn process_groups_into_resource_with_custom_collection(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
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

    #[cfg(feature = "native-http")]
    /// Processes an ordered list of `TextGroup`s into a
    /// a ready-to-go BaseVectorResource. Allows specifying a custom collection function.
    pub fn process_groups_into_resource_blocking_with_custom_collection(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
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
    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources.
    /// This method assumes your text groups already have embeddings generated for them.
    async fn process_new_doc_resource_with_embeddings_already_generated(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let name = ShinkaiFileParser::clean_name(&name);
        let max_embedding_token_count = generator.model_type().max_input_token_count();
        let resource_desc = Self::setup_resource_description(
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

    #[cfg(feature = "native-http")]
    /// Recursively processes all text groups & their sub groups into DocumentResources.
    /// This method assumes your text groups already have embeddings generated for them.
    fn process_new_doc_resource_blocking_with_embeddings_already_generated(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSourceReference,
        parsing_tags: &Vec<DataTag>,
        resource_embedding: Option<Embedding>,
    ) -> Result<BaseVectorResource, VRError> {
        let name = ShinkaiFileParser::clean_name(&name);
        let max_embedding_token_count = generator.model_type().max_input_token_count();
        let resource_desc = Self::setup_resource_description(
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
            let (new_resource_id, metadata, has_sub_groups, new_name) = Self::process_grouped_text(grouped_text);
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

    /// Clean's the file name of auxiliary data (file extension, url in front of file name, etc.)
    pub fn clean_name(name: &str) -> String {
        // Decode URL-encoded characters to simplify processing.
        let decoded_name = urlencoding::decode(name).unwrap_or_else(|_| name.into());

        // Check if the name ends with ".htm" or ".html" and calculate the position to avoid deletion.
        let avoid_deletion_position = if decoded_name.ends_with(".htm") || decoded_name.ends_with(".html") {
            decoded_name.len().saturating_sub(4) // Position before ".htm" or ".html"
        } else {
            decoded_name.len() // Use the full length if not ending with ".htm" or ".html"
        };
        // Find the last occurrence of "/" or "%2F" that is not too close to the ".htm" extension.
        let last_relevant_slash_position = decoded_name.rmatch_indices(&['/', '%']).find_map(|(index, _)| {
            if index + 3 < avoid_deletion_position && decoded_name[index..].starts_with("%2F") {
                Some(index)
            } else if index + 1 < avoid_deletion_position && decoded_name[index..].starts_with("/") {
                Some(index)
            } else {
                None
            }
        });
        // If a relevant slash is found, slice the string from the character immediately following this slash.
        let http_cleaned = match last_relevant_slash_position {
            Some(index) => decoded_name
                .get((index + if decoded_name[index..].starts_with("%2F") { 3 } else { 1 })..)
                .unwrap_or(&decoded_name),
            None => &decoded_name,
        };

        let http_cleaned = if http_cleaned.is_empty() || http_cleaned == ".html" || http_cleaned == ".htm" {
            decoded_name.to_string()
        } else {
            http_cleaned.to_string()
        };

        // Remove extension
        let cleaned_name = SourceFileType::clean_string_of_extension(&http_cleaned);

        cleaned_name
    }

    /// Helper function that processes groups into a list of descriptions.
    /// Only takes the top level Group text, does not traverse deeper.
    pub fn process_groups_into_descriptions_list(
        groups: &Vec<GroupedText>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> Vec<String> {
        let mut descriptions = Vec::new();
        let mut description = String::new();
        let mut total_size = 0;

        for group in groups {
            let element_text = &group.text;
            if description.len() + element_text.len() > max_node_text_size {
                descriptions.push(description.clone());
                total_size += description.len();
                description.clear();
            }
            if total_size + element_text.len() > max_size {
                break;
            }
            description.push_str(element_text);
            description.push(' ');
        }
        if !description.is_empty() {
            descriptions.push(description);
        }

        descriptions
    }

    /// Processes groups into a single description string.
    /// Only takes the top level Group text, does not traverse deeper.
    pub fn process_groups_into_description(
        groups: &Vec<GroupedText>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> String {
        let descriptions = Self::process_groups_into_descriptions_list(groups, max_size, max_node_text_size);
        descriptions.join(" ")
    }

    /// Helper method for setting a description if none provided for process_new_doc_resource
    fn setup_resource_description(
        desc: Option<String>,
        text_groups: &Vec<GroupedText>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> Option<String> {
        if let Some(description) = desc {
            Some(description.to_string())
        } else if !text_groups.is_empty() {
            Some(Self::process_groups_into_description(
                text_groups,
                max_size,
                max_node_text_size,
            ))
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

    /// Attempts to process a file into a list of GroupedTexts using local processing
    /// implemented in Rust directly without relying on external services.
    /// If local processing is not available for provided source, then returns Err.
    pub fn local_process_file_into_grouped_text(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
    ) -> Result<Vec<GroupedText>, VRError> {
        let source_base = source;

        match source_base {
            VRSourceReference::None => Err(VRError::UnsupportedFileType(file_name.to_string())),
            VRSourceReference::Standard(source) => match source {
                SourceReference::Other(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
                SourceReference::FileRef(file_source) => match file_source.file_type {
                    SourceFileType::Document(_)
                    | SourceFileType::Image(_)
                    | SourceFileType::Code(_)
                    | SourceFileType::ConfigFileType(_)
                    | SourceFileType::Video(_)
                    | SourceFileType::Audio(_)
                    | SourceFileType::Shinkai(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
                },
                SourceReference::ExternalURI(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
            },
            VRSourceReference::Notarized(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
        }
    }
}

// /// Parse CSV data from a buffer and attempt to automatically detect
// /// headers.
// pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VRError> {
//     let mut reader = Reader::from_reader(Cursor::new(buffer));
//     let headers = reader
//         .headers()
//         .map_err(|_| VRError::FailedCSVParsing)?
//         .iter()
//         .map(String::from)
//         .collect::<Vec<String>>();

//     let likely_header = headers.iter().all(|s| {
//         let is_alphabetic = s.chars().all(|c| c.is_alphabetic() || c.is_whitespace());
//         let no_duplicates = headers.iter().filter(|&item| item == s).count() == 1;
//         let no_prohibited_chars = !s.contains(&['@', '#', '$', '%', '^', '&', '*'][..]);

//         is_alphabetic && no_duplicates && no_prohibited_chars
//     });

//     Self::parse_csv(&buffer, likely_header)
// }

// /// Parse CSV data from a buffer.
// /// * `header` - A boolean indicating whether to prepend column headers to
// ///   values.
// pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VRError> {
//     let mut reader = Reader::from_reader(Cursor::new(buffer));
//     let headers = if header {
//         reader
//             .headers()
//             .map_err(|_| VRError::FailedCSVParsing)?
//             .iter()
//             .map(String::from)
//             .collect::<Vec<String>>()
//     } else {
//         Vec::new()
//     };

//     let mut result = Vec::new();
//     for record in reader.records() {
//         let record = record.map_err(|_| VRError::FailedCSVParsing)?;
//         let row: Vec<String> = if header {
//             record
//                 .iter()
//                 .enumerate()
//                 .map(|(i, e)| format!("{}: {}", headers[i], e))
//                 .collect()
//         } else {
//             record.iter().map(String::from).collect()
//         };
//         let row_string = row.join(", ");
//         result.push(row_string);
//     }

//     Ok(result)
// }
