use super::unstructured_types::{ElementType, GroupedText, UnstructuredElement};
use crate::base_vector_resources::BaseVectorResource;
use crate::data_tags::DataTag;
use crate::document_resource::DocumentVectorResource;
use crate::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use crate::embeddings::Embedding;
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::VectorResource;
use async_recursion::async_recursion;
use blake3::Hasher;
use keyphrases::KeyPhraseExtractor;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

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

    /// Extracts the most important keywords from a given text,
    /// using the RAKE algorithm.
    pub fn extract_keywords(group: &Vec<GroupedText>, num: u64) -> Vec<String> {
        // Extract all the text out of all the elements and combine them together into a single string
        let text = group
            .iter()
            .map(|element| element.text.as_str())
            .collect::<Vec<&str>>()
            .join(" ");

        // Create a new KeyPhraseExtractor with a maximum of num keywords
        let extractor = KeyPhraseExtractor::new(&text, num as usize);

        // Get the keywords
        let keywords = extractor.get_keywords();

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource
    #[cfg(feature = "native-http")]
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
        // Group elements together before generating the doc
        let text_groups = UnstructuredParser::hierarchical_group_elements_text(&elements, max_chunk_size);
        Self::process_new_doc_resource_blocking(text_groups, generator, &name, desc, source, parsing_tags, &resource_id)
    }

    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource
    /// Note: This is the async variant.
    #[cfg(feature = "native-http")]
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
        // Group elements together before generating the doc
        let text_groups = UnstructuredParser::hierarchical_group_elements_text(&elements, max_chunk_size);
        let task = tokio::spawn(async move {
            let generator = RemoteEmbeddingGenerator::new_default();
            Self::generate_text_group_embeddings(&text_groups, &generator, 31, max_chunk_size).await
        });
        let new_text_groups = task.await??;
        println!("New text groups: {:?}", new_text_groups);

        Self::process_new_doc_resource(
            new_text_groups,
            generator,
            &name,
            desc,
            source,
            parsing_tags,
            &resource_id,
            None,
        )
        .await
    }

    /// Recursively processes all text groups & their sub groups into DocumentResources
    /// Note: This is the async variant.
    #[async_recursion]
    #[cfg(feature = "native-http")]
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

        for grouped_text in &text_groups {
            println!("Processing group text for resource");
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
                    println!("Set already generated embedding");
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

    /// Recursively processes all text groups & their sub groups into DocumentResources
    #[cfg(feature = "native-http")]
    fn process_new_doc_resource_blocking(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        let resource_desc = Self::setup_resource_description(desc, &text_groups);
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc.as_deref(), source.clone(), &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        let keywords = UnstructuredParser::extract_keywords(&text_groups, 50);
        doc.update_resource_embedding_blocking(generator, keywords)?;

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
                )?;
                doc.append_vector_resource(new_doc, metadata);
            } else {
                if grouped_text.text.len() < 6 {
                    continue;
                }
                let embedding = generator.generate_embedding_default_blocking(&grouped_text.text)?;
                doc.append_data(&grouped_text.text, metadata, &embedding, parsing_tags);
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

    /// Recursive function to collect all texts from the text groups and their subgroups
    /// TODO: Extract and add keywords if the text_group has sub-groups before adding the text (use max chunk size to limit how many get added).
    fn collect_texts_and_indices(
        text_groups: &[GroupedText],
        texts: &mut Vec<String>,
        indices: &mut Vec<(Vec<usize>, usize)>,
        max_chunk_size: u64,
        path: Vec<usize>,
    ) {
        for (i, text_group) in text_groups.iter().enumerate() {
            println!("Processing text group at index {}", i);
            texts.push(text_group.text.clone());
            let mut current_path = path.clone();
            current_path.push(i);
            indices.push((current_path.clone(), texts.len() - 1));
            for (j, sub_group) in text_group.sub_groups.iter().enumerate() {
                println!("Processing sub-group at index {} of text group at index {}", j, i);
                texts.push(sub_group.text.clone());
                let mut sub_path = current_path.clone();
                sub_path.push(j);
                indices.push((sub_path, texts.len() - 1));
            }
            for sub_group in &text_group.sub_groups {
                Self::collect_texts_and_indices(
                    &sub_group.sub_groups,
                    texts,
                    indices,
                    max_chunk_size,
                    current_path.clone(),
                );
            }
        }
    }

    /// Recursive function to assign the generated embeddings back to the text groups and their subgroups
    fn assign_embeddings(
        text_groups: &mut [GroupedText],
        embeddings: &mut Vec<Embedding>,
        indices: &[(Vec<usize>, usize)],
    ) {
        for (path, flat_index) in indices {
            if let Some(embedding) = embeddings.get(*flat_index) {
                let mut target = &mut text_groups[path[0]];
                for &index in &path[1..] {
                    target = &mut target.sub_groups[index];
                }
                target.embedding = Some(embedding.clone());
            }
        }
    }

    /// Recursively goes through all of the text groups and batch generates embeddings
    /// for all of them.
    #[async_recursion]
    pub async fn generate_text_group_embeddings(
        text_groups: &Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        mut max_batch_size: u64,
        max_chunk_size: u64,
    ) -> Result<Vec<GroupedText>, VectorResourceError> {
        // Clone the input text_groups
        let mut text_groups = text_groups.clone();

        // Collect all texts from the text groups and their subgroups
        let mut texts = Vec::new();
        let mut indices = Vec::new();
        Self::collect_texts_and_indices(&text_groups, &mut texts, &mut indices, max_chunk_size, vec![]);

        // Generate embeddings for all texts in batches
        let ids: Vec<String> = vec!["".to_string(); texts.len()];
        let mut embeddings = Vec::new();
        for batch in texts.chunks(max_batch_size as usize) {
            let batch_ids = &ids[..batch.len()];
            println!("Generating batched embeddings for {} text groups", batch_ids.len());
            match generator
                .generate_embeddings(&batch.to_vec(), &batch_ids.to_vec())
                .await
            {
                Ok(batch_embeddings) => {
                    embeddings.extend(batch_embeddings);
                }
                Err(e) => {
                    println!("Error generating embeddings: {:?}", e);
                    if max_batch_size > 5 {
                        max_batch_size -= 5;
                        return Self::generate_text_group_embeddings(
                            &text_groups,
                            generator,
                            max_batch_size,
                            max_chunk_size,
                        )
                        .await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        println!("Number of texts: {}", texts.len());
        println!("Number of indices: {}", indices.len());
        println!("Number of embeddings: {}", embeddings.len());

        // Assign the generated embeddings back to the text groups and their subgroups
        Self::assign_embeddings(&mut text_groups, &mut embeddings, &indices);

        println!("Embeddings successfully assigned!");
        Ok(text_groups)
    }

    /// Helper method for processing a grouped text for process_new_doc_resource
    fn process_grouped_text(grouped_text: &GroupedText) -> (String, Option<HashMap<String, String>>, bool, String) {
        let has_sub_groups = !grouped_text.sub_groups.is_empty();
        let new_name = grouped_text.text.clone();
        let new_resource_id = Self::generate_data_hash(new_name.as_bytes());
        let mut metadata = HashMap::new();
        metadata.insert("page_numbers".to_string(), grouped_text.format_page_num_string());
        (new_resource_id, Some(metadata), has_sub_groups, new_name)
    }
    /// Internal method used to push into correct group for hierarchical grouping
    fn push_group_to_appropriate_parent(
        group: GroupedText,
        title_group: &mut Option<GroupedText>,
        groups: &mut Vec<GroupedText>,
    ) {
        if group.text.len() <= 2 {
            return;
        }

        if let Some(title_group) = title_group.as_mut() {
            title_group.push_sub_group(group);
        } else {
            groups.push(group);
        }
    }

    /// Given a list of `UnstructuredElement`s, groups their text together into a hierarchy.
    /// Currently respects max_chunk_size, ensures all content in between title elements are sub-grouped,
    /// and skips over all uncategorized text.
    pub fn hierarchical_group_elements_text(
        elements: &Vec<UnstructuredElement>,
        max_chunk_size: u64,
    ) -> Vec<GroupedText> {
        let max_chunk_size = max_chunk_size as usize;
        let mut groups = Vec::new();
        let mut current_group = GroupedText::new();
        let mut current_title_group: Option<GroupedText> = None;

        // Step 1: Remove duplicate titles to cleanup elements
        let elements = Self::remove_duplicate_title_elements(elements);

        // Step 2: Concatenate the first sequence of consecutive titles (useful for pdfs)
        let mut elements_iter = elements.iter().peekable();
        while let Some(element) = elements_iter.peek() {
            if element.element_type == ElementType::Title
                && current_group.text.len() + element.text.len() < max_chunk_size
                && element.text.len() > 2
            {
                current_group.push_data(&element.text, element.metadata.page_number);
                elements_iter.next(); // advance the iterator
            } else if element.text.len() <= 2 {
                elements_iter.next(); // advance the iterator
            } else {
                break;
            }
        }

        // Step 3: Main loop: process the remaining elements
        for element in elements_iter {
            let element_text = element.text.clone();

            // Skip over any uncategorized text (usually filler like headers/footers)
            // Or elements with no content
            if element.element_type == ElementType::UncategorizedText || element.text.len() <= 2 {
                continue;
            }

            // If adding the current element text would exceed the max_chunk_size,
            // push the current group to title group or groups and start a new group
            if current_group.text.len() + element_text.len() > max_chunk_size {
                Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
                current_group = GroupedText::new();
            }

            // If the current element text is larger than max_chunk_size,
            // split it into chunks and add them to title group or groups
            if element_text.len() > max_chunk_size {
                let chunks = Self::split_into_chunks(&element_text, max_chunk_size);
                for chunk in chunks {
                    let mut new_group = GroupedText::new();
                    new_group.push_data(&chunk, element.metadata.page_number);
                    Self::push_group_to_appropriate_parent(new_group, &mut current_title_group, &mut groups);
                }
                continue;
            }

            // Add the current element text to the current group
            current_group.push_data(&element_text, element.metadata.page_number);

            // If the current element type is Title,
            // push the current title group to groups and start a new title group
            if element.element_type == ElementType::Title {
                // Add the current group to the existing title group only if it contains more than the title text
                if !current_group.text.is_empty() && current_group.text != element_text {
                    Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
                } else if let Some(title_group) = current_title_group.as_mut() {
                    // If the current group only contains the title text, add a default GroupedText that holds the title's text
                    // This both pre-populates the sub-group field, and allows for the title to be found in a search
                    // as a RetrievedDataChunk to the LLM which can be useful in some content.
                    let mut new_grouped_text = GroupedText::new();
                    new_grouped_text.push_data(&title_group.text, None);
                    title_group.sub_groups.push(new_grouped_text);
                }
                current_group = GroupedText::new();

                // Push the existing title group to groups
                if let Some(title_group) = current_title_group.take() {
                    groups.push(title_group);
                }

                // Start a new title group
                current_title_group = Some(GroupedText::new());
                if let Some(title_group) = current_title_group.as_mut() {
                    title_group.push_data(&element_text, element.metadata.page_number);
                }
                continue;
            }
        }

        // Push the last group to title group or groups
        if !current_group.text.len() < 2 {
            Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
        }
        // Push the last title group to groups
        if let Some(title_group) = current_title_group.take() {
            groups.push(title_group);
        }

        // Filter out groups with a text of 15 characters or less and no sub-groups
        groups = groups
            .into_iter()
            .filter(|group| group.text.len() > 15 || !group.sub_groups.is_empty())
            .collect();

        groups
    }

    /// Removes any title element which occurs more than once.
    /// Useful especially for PDFs/docs where headers/footers repeat
    /// and Unstructured failed to separate them out as Uncategorized.
    pub fn remove_duplicate_title_elements(elements: &Vec<UnstructuredElement>) -> Vec<UnstructuredElement> {
        let mut title_counts = HashMap::new();
        let mut result = Vec::new();

        // First pass: count the occurrences of each title
        for element in elements {
            if element.element_type == ElementType::Title {
                *title_counts.entry(&element.text).or_insert(0) += 1;
            }
        }

        // Second pass: build the result, skipping titles that appear more than once
        for element in elements {
            if element.element_type == ElementType::Title {
                if title_counts[&element.text] > 1 {
                    continue;
                }
            }
            result.push(element.clone());
        }

        result
    }

    /// Splits a string into chunks at the nearest whitespace to a given size
    pub fn split_into_chunks(text: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = start + chunk_size;
            let end = if end < text.len() {
                let mut end = end;
                while end > start && !text.as_bytes()[end].is_ascii_whitespace() {
                    end -= 1;
                }
                if end == start {
                    start + chunk_size
                } else {
                    end
                }
            } else {
                text.len()
            };

            let chunk = &text[start..end];
            chunks.push(chunk.to_string());

            start = end;
        }

        chunks
    }

    /// Generates a Blake3 hash of the data in the buffer
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }
}
