use super::file_parser::ShinkaiFileParser;
use super::file_parser_types::TextGroup;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
#[cfg(feature = "desktop-only")]
use async_recursion::async_recursion;
use keyphrases::KeyPhraseExtractor;
use regex::Regex;
use std::collections::HashMap;

impl ShinkaiFileParser {
    /// Recursive function to collect all texts from the text groups and their subgroups
    pub fn collect_texts_and_indices(
        text_groups: &[TextGroup],
        max_node_text_size: u64,
        path: Vec<usize>,
    ) -> (Vec<String>, Vec<(Vec<usize>, usize)>) {
        let mut texts = Vec::new();
        let mut indices = Vec::new();

        for (i, text_group) in text_groups.iter().enumerate() {
            texts.push(text_group.format_text_for_embedding(max_node_text_size));
            let mut current_path = path.clone();
            current_path.push(i);
            indices.push((current_path.clone(), texts.len() - 1));

            for (j, sub_group) in text_group.sub_groups.iter().enumerate() {
                texts.push(sub_group.format_text_for_embedding(max_node_text_size));
                let mut sub_path = current_path.clone();
                sub_path.push(j);
                indices.push((sub_path.clone(), texts.len() - 1));

                let (sub_texts, sub_indices) =
                    Self::collect_texts_and_indices(&sub_group.sub_groups, max_node_text_size, sub_path);
                texts.extend(sub_texts);
                indices.extend(sub_indices);
            }
        }

        (texts, indices)
    }

    /// Recursive function to assign the generated embeddings back to the text groups and their subgroups
    fn assign_embeddings(
        text_groups: &mut [TextGroup],
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

    #[cfg(feature = "desktop-only")]
    #[async_recursion]
    /// Recursively goes through all of the text groups and batch generates embeddings
    /// for all of them in parallel, processing up to 10 futures at a time.
    pub async fn generate_text_group_embeddings(
        text_groups: &Vec<TextGroup>,
        generator: Box<dyn EmbeddingGenerator>,
        mut max_batch_size: u64,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
    ) -> Result<Vec<TextGroup>, VRError> {
        // Clone the input text_groups
        let mut text_groups = text_groups.clone();

        // Collect all texts from the text groups and their subgroups
        let (texts, indices) = collect_texts_and_indices(&text_groups, max_node_text_size, vec![]);

        // Generate embeddings for all texts in batches
        let ids: Vec<String> = vec!["".to_string(); texts.len()];
        let mut all_futures = Vec::new();
        let mut current_batch_futures = Vec::new();

        for (index, batch) in texts.chunks(max_batch_size as usize).enumerate() {
            let batch_texts = batch.to_vec();
            let batch_ids = ids[..batch.len()].to_vec();
            let generator_clone = generator.box_clone(); // Clone the generator for use in the future.

            // Use the `move` keyword to take ownership of `generator_clone` inside the async block.
            let future = async move { generator_clone.generate_embeddings(&batch_texts, &batch_ids).await };
            current_batch_futures.push(future);

            // If we've collected 10 futures or are at the last batch, add them to all_futures and start a new vector
            if current_batch_futures.len() == 10 || index == texts.chunks(max_batch_size as usize).count() - 1 {
                all_futures.push(current_batch_futures);
                current_batch_futures = Vec::new();
            }
        }

        // Process each group of up to 10 futures in sequence
        let mut embeddings = Vec::new();
        for futures_group in all_futures {
            let results = futures::future::join_all(futures_group).await;
            for result in results {
                match result {
                    Ok(batch_embeddings) => {
                        embeddings.extend(batch_embeddings);
                    }
                    Err(e) => {
                        if max_batch_size > 5 {
                            max_batch_size -= 5;
                            return Self::generate_text_group_embeddings(
                                &text_groups,
                                generator,
                                max_batch_size,
                                max_node_text_size,
                                collect_texts_and_indices,
                            )
                            .await;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Assign the generated embeddings back to the text groups and their subgroups
        Self::assign_embeddings(&mut text_groups, &mut embeddings, &indices);

        Ok(text_groups)
    }

    #[cfg(feature = "desktop-only")]
    /// Recursively goes through all of the text groups and batch generates embeddings
    /// for all of them.
    pub fn generate_text_group_embeddings_blocking(
        text_groups: &Vec<TextGroup>,
        generator: Box<dyn EmbeddingGenerator>,
        mut max_batch_size: u64,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
    ) -> Result<Vec<TextGroup>, VRError> {
        // Clone the input text_groups
        let mut text_groups = text_groups.clone();

        // Collect all texts from the text groups and their subgroups
        let (texts, indices) = collect_texts_and_indices(&text_groups, max_node_text_size, vec![]);

        // Generate embeddings for all texts in batches
        let ids: Vec<String> = vec!["".to_string(); texts.len()];
        let mut embeddings = Vec::new();
        for batch in texts.chunks(max_batch_size as usize) {
            let batch_ids = &ids[..batch.len()];
            match generator.generate_embeddings_blocking(&batch.to_vec(), &batch_ids.to_vec()) {
                Ok(batch_embeddings) => {
                    embeddings.extend(batch_embeddings);
                }
                Err(e) => {
                    if max_batch_size > 5 {
                        max_batch_size -= 5;
                        return Self::generate_text_group_embeddings_blocking(
                            &text_groups,
                            generator,
                            max_batch_size,
                            max_node_text_size,
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
    pub fn process_grouped_text(grouped_text: &TextGroup) -> (String, Option<HashMap<String, String>>, bool, String) {
        let has_sub_groups = !grouped_text.sub_groups.is_empty();
        let new_name = grouped_text.text.clone();
        let new_resource_id = Self::generate_data_hash(new_name.as_bytes());

        let metadata = grouped_text.metadata.clone();

        (new_resource_id, Some(metadata), has_sub_groups, new_name)
    }

    /// Internal method used to push into correct group for hierarchical grouping
    pub fn push_group_to_appropriate_parent(
        group: TextGroup,
        title_group: &mut Option<TextGroup>,
        groups: &mut Vec<TextGroup>,
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

    /// Splits a string into chunks at the nearest whitespace to a given size avoiding splitting metadata
    pub fn split_into_chunks_with_metadata(text: &str, chunk_size: usize) -> Vec<String> {
        // The regex matches both pure and replaceable metadata
        let re = Regex::new(Self::METADATA_REGEX).unwrap();
        let matched_positions: Vec<(usize, usize)> = re.find_iter(text).map(|m| (m.start(), m.end())).collect();

        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = start + chunk_size;
            let end = if end < text.len() {
                let mut end = end;
                while end > start
                    && (!text.as_bytes()[end].is_ascii_whitespace()
                        || matched_positions
                            .iter()
                            .any(|(meta_start, meta_end)| end >= *meta_start && end < *meta_end))
                {
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
    pub fn extract_keywords(groups: &Vec<TextGroup>, num: u64) -> Vec<String> {
        // Extract all the text out of all the TextGroup and its subgroups and combine them together into a single string
        let text = Self::extract_all_text_from_groups(groups);

        // Create a new KeyPhraseExtractor with a maximum of num keywords
        let extractor = KeyPhraseExtractor::new(&text, num as usize);

        // Get the keywords
        let keywords = extractor.get_keywords();

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Extracts all  text from the list of groups and any sub-groups inside
    fn extract_all_text_from_groups(group: &Vec<TextGroup>) -> String {
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

    /// Concatenate text up to a maximum size.
    pub fn concatenate_groups_up_to_max_size(elements: &Vec<TextGroup>, max_size: usize) -> String {
        let mut desc = String::new();
        for e in elements {
            if desc.len() + e.text.len() + 1 > max_size {
                break; // Stop appending if adding the next element would exceed max_size
            }
            desc.push_str(&e.text);
            desc.push('\n'); // Add a line break after each element's text
        }
        desc.trim_end().to_string() // Trim any trailing space before returning
    }
}
