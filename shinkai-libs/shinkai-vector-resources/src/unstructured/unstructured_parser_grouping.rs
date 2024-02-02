use super::unstructured_parser::UnstructuredParser;
use super::unstructured_types::{ElementType, GroupedText, UnstructuredElement};
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
#[cfg(feature = "native-http")]
use async_recursion::async_recursion;
use keyphrases::KeyPhraseExtractor;
use std::collections::HashMap;

impl UnstructuredParser {
    /// Recursive function to collect all texts from the text groups and their subgroups
    pub fn collect_texts_and_indices(
        text_groups: &[GroupedText],
        texts: &mut Vec<String>,
        indices: &mut Vec<(Vec<usize>, usize)>,
        max_chunk_size: u64,
        path: Vec<usize>,
    ) {
        for (i, text_group) in text_groups.iter().enumerate() {
            println!("Processing text group at index {}", i);
            texts.push(text_group.format_text_for_embedding(max_chunk_size));
            let mut current_path = path.clone();
            current_path.push(i);
            indices.push((current_path.clone(), texts.len() - 1));
            for (j, sub_group) in text_group.sub_groups.iter().enumerate() {
                texts.push(sub_group.format_text_for_embedding(max_chunk_size));
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

    #[cfg(feature = "native-http")]
    #[async_recursion]
    /// Recursively goes through all of the text groups and batch generates embeddings
    /// for all of them.
    pub async fn generate_text_group_embeddings(
        text_groups: &Vec<GroupedText>,
        generator: Box<dyn EmbeddingGenerator>,
        mut max_batch_size: u64,
        max_chunk_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
    ) -> Result<Vec<GroupedText>, VRError> {
        // Clone the input text_groups
        let mut text_groups = text_groups.clone();

        // Collect all texts from the text groups and their subgroups
        let mut texts = Vec::new();
        let mut indices = Vec::new();
        collect_texts_and_indices(&text_groups, &mut texts, &mut indices, max_chunk_size, vec![]);

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
                            collect_texts_and_indices,
                        )
                        .await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // Assign the generated embeddings back to the text groups and their subgroups
        Self::assign_embeddings(&mut text_groups, &mut embeddings, &indices);

        Ok(text_groups)
    }

    #[cfg(feature = "native-http")]
    /// Recursively goes through all of the text groups and batch generates embeddings
    /// for all of them.
    pub fn generate_text_group_embeddings_blocking(
        text_groups: &Vec<GroupedText>,
        generator: Box<dyn EmbeddingGenerator>,
        mut max_batch_size: u64,
        max_chunk_size: u64,
        collect_texts_and_indices: fn(&[GroupedText], &mut Vec<String>, &mut Vec<(Vec<usize>, usize)>, u64, Vec<usize>),
    ) -> Result<Vec<GroupedText>, VRError> {
        // Clone the input text_groups
        let mut text_groups = text_groups.clone();

        // Collect all texts from the text groups and their subgroups
        let mut texts = Vec::new();
        let mut indices = Vec::new();
        collect_texts_and_indices(&text_groups, &mut texts, &mut indices, max_chunk_size, vec![]);

        // Generate embeddings for all texts in batches
        let ids: Vec<String> = vec!["".to_string(); texts.len()];
        let mut embeddings = Vec::new();
        for batch in texts.chunks(max_batch_size as usize) {
            let batch_ids = &ids[..batch.len()];
            println!("Generating batched embeddings for {} text groups", batch_ids.len());
            match generator.generate_embeddings_blocking(&batch.to_vec(), &batch_ids.to_vec()) {
                Ok(batch_embeddings) => {
                    embeddings.extend(batch_embeddings);
                }
                Err(e) => {
                    println!("Error generating embeddings: {:?}", e);
                    if max_batch_size > 5 {
                        max_batch_size -= 5;
                        return Self::generate_text_group_embeddings_blocking(
                            &text_groups,
                            generator,
                            max_batch_size,
                            max_chunk_size,
                            collect_texts_and_indices,
                        );
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // Assign the generated embeddings back to the text groups and their subgroups
        Self::assign_embeddings(&mut text_groups, &mut embeddings, &indices);

        Ok(text_groups)
    }

    /// Helper method for processing a grouped text for process_new_doc_resource
    pub fn process_grouped_text(grouped_text: &GroupedText) -> (String, Option<HashMap<String, String>>, bool, String) {
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

            // Pre-parsing to skip useless elements that Unstructured failed to clean out itself properly
            if Self::should_element_be_skipped(element) {
                continue;
            }

            if element.element_type != ElementType::Title {
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
            }

            // If the current element type is Title,
            // push the current title group to groups and start a new title group
            if element.element_type == ElementType::Title {
                // Add the current group to the existing title group only if it contains more than the title text
                if !current_group.text.is_empty() && current_group.text != element_text {
                    Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
                } else if let Some(title_group) = current_title_group.as_mut() {
                    if element_text.len() > 12 {
                        // If the current group only contains the title text and is > 12 len, add a default GroupedText that holds the title's text
                        // This both pre-populates the sub-group field, and allows for the title to be found in a search
                        // as a RetrievedNode to the LLM which can be useful in some content.
                        let mut new_grouped_text = GroupedText::new();
                        new_grouped_text.push_data(&title_group.text, None);
                        title_group.sub_groups.push(new_grouped_text);
                    }
                }
                current_group = GroupedText::new();

                // Push the existing title group to groups
                if let Some(title_group) = current_title_group.take() {
                    if title_group.text.len() > 12 || title_group.sub_groups.len() > 0 {
                        groups.push(title_group);
                    }
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

        groups
    }

    /// Skip over any elements that Unstructured failed to clean out.
    fn should_element_be_skipped(element: &UnstructuredElement) -> bool {
        // Remove Uncategorized text (usually filler like headers/footers) && elements with no content at all.
        if element.element_type == ElementType::UncategorizedText || element.text.len() <= 2 {
            return true;
        }

        // Remove short narrative text which doesn't have enough content to matter
        if element.element_type == ElementType::NarrativeText && element.text.len() <= 12 {
            return true;
        }

        // For pieces of codeblocks which Unstructured failed to parse together and split up horribly.
        if !element.text.contains(' ')
            && (element.text.contains('.')
                || element.text.contains('_')
                || element.text.contains('[')
                || element.text.contains(']')
                || element.text.contains("::"))
        {
            return true;
        }

        false
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

    /// Extracts the most important keywords from all Groups/Sub-groups
    /// using the RAKE algorithm.
    pub fn extract_keywords(groups: &Vec<GroupedText>, num: u64) -> Vec<String> {
        // Extract all the text out of all the GroupedText and its subgroups and combine them together into a single string
        let text = Self::extract_all_text_from_groups(groups);

        // Create a new KeyPhraseExtractor with a maximum of num keywords
        let extractor = KeyPhraseExtractor::new(&text, num as usize);

        // Get the keywords
        let keywords = extractor.get_keywords();

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Extracts all  text from the list of groups and any sub-groups inside
    fn extract_all_text_from_groups(group: &Vec<GroupedText>) -> String {
        group
            .iter()
            .map(|element| {
                let mut text = element.text.clone();
                for sub_group in &element.sub_groups {
                    text.push_str(&Self::extract_all_text_from_groups(&vec![sub_group.clone()]));
                }
                text
            })
            .collect::<Vec<String>>()
            .join(" ")
    }
}
