use keyphrases::KeyPhraseExtractor;
use regex::Regex;
use shinkai_embedding::embedding_generator::EmbeddingGenerator;

use std::{future::Future, pin::Pin};

use crate::shinkai_fs_error::ShinkaiFsError;

use super::file_parser_helper::ShinkaiFileParser;
use super::text_group::TextGroup;

impl ShinkaiFileParser {
    /// Collect all texts from the TextGroups in a single dimension (no subgroups).
    /// Returns a tuple of:
    ///  - `Vec<String>` for all text
    ///  - `Vec<(Vec<usize>, usize)>` for the “paths” (here just `[i]`) and text index
    pub fn collect_texts_and_indices(
        text_groups: &[TextGroup],
        max_node_text_size: u64,
        path: Vec<usize>,
    ) -> (Vec<String>, Vec<(Vec<usize>, usize)>) {
        let mut texts = Vec::new();
        let mut indices = Vec::new();

        for (i, text_group) in text_groups.iter().enumerate() {
            // Format text with your metadata or keyword logic
            let formatted_text = text_group.format_text_for_embedding(max_node_text_size);
            texts.push(formatted_text);

            // Build a “path” that refers to just the top-level group (no subgroups).
            let mut current_path = path.clone();
            current_path.push(i);
            // The last text we pushed is at index `texts.len() - 1`
            indices.push((current_path, texts.len() - 1));
        }

        (texts, indices)
    }

    /// Assign generated embeddings back into the TextGroups (single dimension).
    fn assign_embeddings(
        text_groups: &mut [TextGroup],
        embeddings: &mut Vec<Vec<f32>>,
        indices: &[(Vec<usize>, usize)],
    ) {
        for (path, text_idx) in indices {
            // We expect path = [i], but if you store deeper paths, you can interpret them differently.
            let i = path[0];
            if let Some(embedding) = embeddings.get(*text_idx) {
                text_groups[i].embedding = Some(embedding.clone());
            }
        }
    }

    /// Batch-generate embeddings for all the TextGroups (no subgroups).
    pub fn generate_text_group_embeddings(
        text_groups: Vec<TextGroup>,
        generator: Box<dyn EmbeddingGenerator>,
        mut max_batch_size: u64,
        max_node_text_size: u64,
        collect_texts_and_indices: fn(&[TextGroup], u64, Vec<usize>) -> (Vec<String>, Vec<(Vec<usize>, usize)>),
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TextGroup>, ShinkaiFsError>> + Send>> {
        Box::pin(async move {
            // Make a mutable copy of the incoming text groups
            let mut text_groups = text_groups;

            // Collect all texts (flattened) from the text groups
            let (texts, indices) = collect_texts_and_indices(&text_groups, max_node_text_size, vec![]);

            // Prepare to generate embeddings in batches
            let mut embeddings = Vec::new();
            let mut all_futures = Vec::new();
            let mut current_batch_futures = Vec::new();

            // Break texts into chunks of size `max_batch_size`
            for (index, batch) in texts.chunks(max_batch_size as usize).enumerate() {
                let batch_texts = batch.to_vec();
                let generator_clone = generator.box_clone(); // clone for an async block below

                let future = async move {
                    generator_clone.generate_embeddings(&batch_texts).await
                };
                current_batch_futures.push(future);

                // If we have 10 futures queued or we're at the last batch, we gather them
                if current_batch_futures.len() == 10 ||
                   index == texts.chunks(max_batch_size as usize).count() - 1
                {
                    all_futures.push(current_batch_futures);
                    current_batch_futures = Vec::new();
                }
            }

            // Run each group of futures in sequence
            for futures_group in all_futures {
                // Wait for them all to complete
                let results = futures::future::join_all(futures_group).await;
                for result in results {
                    match result {
                        Ok(batch_embeddings) => embeddings.extend(batch_embeddings),
                        Err(e) => {
                            // Attempt to reduce batch size and retry
                            if max_batch_size > 5 {
                                max_batch_size -= 5;
                                return Self::generate_text_group_embeddings(
                                    text_groups,
                                    generator,
                                    max_batch_size,
                                    max_node_text_size,
                                    collect_texts_and_indices,
                                )
                                .await;
                            } else {
                                return Err(ShinkaiFsError::FailedEmbeddingGeneration(e.to_string()));
                            }
                        }
                    }
                }
            }

            // Assign embeddings back to the flattened text_groups
            Self::assign_embeddings(&mut text_groups, &mut embeddings, &indices);
            Ok(text_groups)
        })
    }

    /// Splits a string into chunks at the nearest whitespace to a given size
    pub fn split_into_chunks(text: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = {
                let mut candidate_end = start + chunk_size;
                if candidate_end >= text.len() {
                    text.len()
                } else {
                    // walk backward until whitespace (or until we reach start)
                    while candidate_end > start && !text.as_bytes()[candidate_end].is_ascii_whitespace() {
                        candidate_end -= 1;
                    }
                    if candidate_end == start {
                        start + chunk_size.min(text.len() - start)
                    } else {
                        candidate_end
                    }
                }
            };
            let chunk = &text[start..end];
            chunks.push(chunk.to_string());
            start = end;
        }
        chunks
    }

    /// Splits a string into chunks at the nearest whitespace to a given size, avoiding splitting metadata.
    pub fn split_into_chunks_with_metadata(text: &str, chunk_size: usize) -> Vec<String> {
        // The regex matches both pure and replaceable metadata.
        let re = Regex::new(Self::METADATA_REGEX).unwrap();
        let matched_positions: Vec<(usize, usize)> = re.find_iter(text).map(|m| (m.start(), m.end())).collect();

        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = {
                let mut candidate_end = start + chunk_size;
                if candidate_end >= text.len() {
                    text.len()
                } else {
                    // walk backward until whitespace or until we exit a metadata block
                    while candidate_end > start &&
                        (
                            !text.as_bytes()[candidate_end].is_ascii_whitespace() ||
                            matched_positions.iter().any(|&(s, e)| candidate_end >= s && candidate_end < e)
                        )
                    {
                        candidate_end -= 1;
                    }
                    if candidate_end == start {
                        start + chunk_size.min(text.len() - start)
                    } else {
                        candidate_end
                    }
                }
            };

            let chunk = &text[start..end];
            chunks.push(chunk.to_string());
            start = end;
        }
        chunks
    }

    /// Extracts the most important keywords from all TextGroups using the RAKE algorithm.
    pub fn extract_keywords(groups: &Vec<TextGroup>, num: u64) -> Vec<String> {
        // Flatten the text from all groups into one string
        let text = groups
            .iter()
            .map(|element| element.text.clone())
            .collect::<Vec<String>>()
            .join(" ");

        // Create a KeyPhraseExtractor with a maximum of `num` keywords
        let extractor = KeyPhraseExtractor::new(&text, num as usize);

        // Return keywords only, discarding scores
        extractor.get_keywords()
                 .into_iter()
                 .map(|(_score, keyword)| keyword)
                 .collect()
    }

    /// Concatenate text from multiple groups up to a maximum size.
    pub fn concatenate_groups_up_to_max_size(elements: &Vec<TextGroup>, max_size: usize) -> String {
        let mut desc = String::new();
        for e in elements {
            if desc.len() + e.text.len() + 1 > max_size {
                break;
            }
            desc.push_str(&e.text);
            desc.push('\n');
        }
        desc.trim_end().to_string()
    }
}
