use blake3::Hasher;
use csv::Reader;
use keyphrases::KeyPhraseExtractor;
use lazy_static::lazy_static;
use mupdf::pdf::PdfDocument;
use regex::Regex;
use sha2::{Digest, Sha256};
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use shinkai_vector_resources::source::{SourceDocumentType, SourceFileType};
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::unstructured::unstructured_parser::UnstructuredParser;
use shinkai_vector_resources::unstructured::unstructured_types::UnstructuredElement;
use shinkai_vector_resources::vector_resource::VectorResource;
use shinkai_vector_resources::{data_tags::DataTag, source::VRSource};
use std::convert::TryInto;
use std::fs;
use std::sync::Arc;
use std::{io::Cursor, vec};
use tokio::sync::Mutex;

use crate::db::ShinkaiDB;

use super::agent::Agent;
use super::error::AgentError;
use super::execution::job_prompts::{JobPromptGenerator, Prompt};
use super::job_manager::JobManager;

lazy_static! {
    pub static ref UNSTRUCTURED_API_URL: &'static str = "https://internal.shinkai.com/";
}

impl JobManager {
    /// Makes an async request to process a file in a buffer to Unstructured server,
    /// and then processing the returned results into a BaseVectorResource
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource(
        db: Arc<Mutex<ShinkaiDB>>,
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        parsing_tags: &Vec<DataTag>,
        agent: SerializedAgent,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, AgentError> {
        // Parse file into needed data
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let unstructured_api = UnstructuredAPI::new(UNSTRUCTURED_API_URL.to_string(), None);
        let source = VRSource::from_file(&name, &file_buffer)?;
        let elements = unstructured_api.file_request(file_buffer, &name).await?;

        // Automatically generate description if none is provided
        let mut desc = desc;
        if desc.is_none() {
            let prompt = ParsingHelper::process_elements_into_description_prompt(&elements, 2000);
            desc = Some(ParsingHelper::ending_stripper(
                &JobManager::inference_agent_and_extract(agent.clone(), prompt, "answer").await?,
            ));
            eprintln!("LLM Generated File Description: {:?}", desc);
        }

        let resource = UnstructuredParser::process_elements_into_resource(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            resource_id,
            max_chunk_size,
        )
        .await?;

        println!("Processed resource");

        resource
            .as_trait_object()
            .print_all_data_chunks_exhaustive(None, true, false);

        Ok(resource)
    }
}

pub struct ParsingHelper {}

pub struct SmartPdfOverview {
    pub keywords: Vec<String>,
    pub description: String,
    pub blake3_hash: String,
    pub grouped_text_list: Vec<String>,
}

impl ParsingHelper {
    /// Generates Blake3 hash of the input data.
    fn generate_data_hash_blake3(content: &[u8]) -> String {
        UnstructuredParser::generate_data_hash(content)
    }

    /// Takes the provided elements and creates a description prompt ready to be used
    /// to inference with an LLM.
    pub fn process_elements_into_description_prompt(elements: &Vec<UnstructuredElement>, max_size: usize) -> Prompt {
        let max_chunk_size = 300;
        let mut descriptions = Vec::new();
        let mut description = String::new();
        let mut total_size = 0;

        for element in elements {
            let element_text = &element.text;
            if description.len() + element_text.len() > max_chunk_size {
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
        JobPromptGenerator::simple_doc_description(descriptions)
    }

    /// Removes last sentence from a string if it contains any of the unwanted phrases.
    /// This is used because the LLM sometimes answers properly, but then adds useless last sentence such as
    /// "However, specific details are not provided in the content." at the end.
    pub fn ending_stripper(string: &str) -> String {
        let mut sentences: Vec<&str> = string.split('.').collect();

        let unwanted_phrases = [
            "however,",
            "unfortunately",
            "additional research",
            "futher research",
            "may be required",
            "i do not",
            "further information",
            "specific details",
            "provided content",
            "more information",
            "not available",
        ];

        while let Some(last_sentence) = sentences.pop() {
            if last_sentence.trim().is_empty() {
                continue;
            }
            let sentence = last_sentence.trim_start().to_lowercase();
            if !unwanted_phrases.iter().any(|&phrase| sentence.contains(phrase)) {
                sentences.push(last_sentence);
            }
            break;
        }

        sentences.join(".")
    }
}

impl ParsingHelper {
    /// Parse CSV data from a buffer and attempt to automatically detect
    /// headers.
    pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VectorResourceError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = reader
            .headers()
            .map_err(|_| VectorResourceError::FailedCSVParsing)?
            .iter()
            .map(String::from)
            .collect::<Vec<String>>();

        let likely_header = headers.iter().all(|s| {
            let is_alphabetic = s.chars().all(|c| c.is_alphabetic() || c.is_whitespace());
            let no_duplicates = headers.iter().filter(|&item| item == s).count() == 1;
            let no_prohibited_chars = !s.contains(&['@', '#', '$', '%', '^', '&', '*'][..]);

            is_alphabetic && no_duplicates && no_prohibited_chars
        });

        Self::parse_csv(&buffer, likely_header)
    }

    /// Parse CSV data from a buffer.
    /// * `header` - A boolean indicating whether to prepend column headers to
    ///   values.
    pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VectorResourceError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = if header {
            reader
                .headers()
                .map_err(|_| VectorResourceError::FailedCSVParsing)?
                .iter()
                .map(String::from)
                .collect::<Vec<String>>()
        } else {
            Vec::new()
        };

        let mut result = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| VectorResourceError::FailedCSVParsing)?;
            let row: Vec<String> = if header {
                record
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}: {}", headers[i], e))
                    .collect()
            } else {
                record.iter().map(String::from).collect()
            };
            let row_string = row.join(", ");
            result.push(row_string);
        }

        Ok(result)
    }

    /// Parse text from a PDF in a buffer, performing sentence extraction,
    /// text cleanup, and sentence grouping (for data chunks).
    /// * `average_group_size` - Average number of characters per group desired.
    ///   Do note, we stop at fully sentences, so this is just a target minimum.
    pub fn parse_pdf_to_string_list(
        buffer: &[u8],
        average_group_size: u64,
    ) -> Result<Vec<String>, VectorResourceError> {
        // Setting average length to 400, to respect small context size LLMs.
        // Sentences continue past this light 400 cap, so it has to be less than the
        // hard cap.
        let num_characters = if average_group_size > 400 {
            400
        } else {
            average_group_size
        };
        let text = ParsingHelper::extract_text_from_pdf_buffer(buffer, None)
            .map_err(|_| VectorResourceError::FailedPDFParsing)?;
        let grouped_text_list = ParsingHelper::split_into_groups(&text, num_characters as usize)?;

        // TODO: remove this. only for testing purposes
        // Convert the Vec<String> into a single String with each element on a new line
        let grouped_clone = grouped_text_list.clone();
        let output = grouped_clone.join("\n");

        // Write the output to a text file
        fs::write("parse_pdf_to_string_list_output.txt", output).expect("Unable to write to file");

        Ok(grouped_text_list)
    }

    pub fn parse_pdf_text_to_string_list(
        text: String,
        average_group_size: u64,
    ) -> Result<Vec<String>, VectorResourceError> {
        // Setting average length to 400, to respect small context size LLMs.
        // Sentences continue past this light 400 cap, so it has to be less than the
        // hard cap.
        let num_characters = if average_group_size > 400 {
            400
        } else {
            average_group_size
        };
        let grouped_text_list = ParsingHelper::split_into_groups(&text, num_characters as usize)?;

        // TODO: remove this. only for testing purposes
        // Convert the Vec<String> into a single String with each element on a new line
        let grouped_clone = grouped_text_list.clone();
        let output = grouped_clone.join("\n");

        // Write the output to a text file
        fs::write("parse_pdf_to_string_list_output.txt", output).expect("Unable to write to file");

        Ok(grouped_text_list)
    }

    /// Extracts text from a pdf buffer using MuPDF
    fn extract_text_from_pdf_buffer(buffer: &[u8], max_pages: Option<i32>) -> Result<String, mupdf::Error> {
        let document = PdfDocument::from_bytes(buffer)?;

        let mut text = String::new();

        let page_limit = max_pages.unwrap_or(document.page_count()?);

        for page_number in 0..std::cmp::min(document.page_count()?, page_limit) {
            let page = document.load_page(page_number)?;
            let page_text = page.to_text()?;
            text.push_str(&page_text);
        }

        Ok(text)
    }

    /// Extracts the most important keywords from a given text,
    /// using the RAKE algorithm.
    pub fn extract_keywords(text: &str, num: u64) -> Vec<String> {
        // Create a new KeyPhraseExtractor with a maximum of num keywords
        let extractor = KeyPhraseExtractor::new(text, num as usize);

        // Get the keywords
        let keywords = extractor.get_keywords();

        // Printing logic
        // keywords
        //     .iter()
        //     .for_each(|(score, keyword)| println!("{}: {}", keyword, score));

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Cleans the input text by performing several operations:
    ///
    /// 1. Replaces newline characters with spaces.
    /// 2. Removes characters that are not alphanumeric, whitespace, or common
    /// punctuation. 3. Replaces sequences of two or more whitespace
    /// characters with a single space. 4. Replaces sequences of periods
    /// followed by whitespace and a digit with a single period and a space.
    /// 5. Removes whitespace before punctuation.
    fn clean_text(text: &str) -> Result<String, VectorResourceError> {
        let text = text.replace("\n", " ");
        let re = Regex::new(r#"[^a-zA-Z0-9 .,!?'\"-$/&@*()\[\]%#]"#)?;
        let re_whitespace = Regex::new(r"\s{2,}")?;
        let re_redundant_periods = Regex::new(r"\.+\s+\d*\.+\s+")?;
        let re_whitespace_before_punctuation = Regex::new(r#"\s([.,!?)\]])"#)?;
        let cleaned_text = re.replace_all(&text, " ");
        let cleaned_text_no_consecutive_spaces = re_whitespace.replace_all(&cleaned_text, " ");
        let cleaned_text_no_redundant_periods =
            re_redundant_periods.replace_all(&cleaned_text_no_consecutive_spaces, ". ");
        let cleaned_text_no_whitespace_before_punctuation =
            re_whitespace_before_punctuation.replace_all(&cleaned_text_no_redundant_periods, "$1");
        Ok(cleaned_text_no_whitespace_before_punctuation
            .to_string()
            .trim()
            .to_owned())
    }

    /// Splits the input text into sentences.
    ///
    /// A sentence is defined as a sequence of characters that ends with a
    /// period, question mark, or exclamation point. However, a period is
    /// not treated as the end of a sentence if it is preceded by a digit or if
    /// it is part of the abbreviations "i.e" or "e.g" or is an email. Sentences that are
    /// less than 10 characters long are not included.
    fn split_into_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut start = 0;
        let mut prev_char_is_digit = false;
        let mut prev_chars = String::new();
        let mut in_email = false;

        for (i, char) in text.char_indices() {
            prev_chars.push(char);
            if prev_chars.len() > 4 {
                prev_chars.remove(0);
            }
            if char == '@' {
                in_email = true;
            }
            if in_email && char.is_whitespace() {
                in_email = false;
            }
            if !in_email
                && ((char == '.'
                    && !prev_char_is_digit
                    && !prev_chars.ends_with("i.e")
                    && !prev_chars.ends_with("e.g"))
                    || char == '?'
                    || char == '!')
            {
                let mut sentence = text[start..i + 1].trim().to_string();
                while sentence.starts_with(',')
                    || sentence.starts_with(')')
                    || sentence.starts_with('.')
                    || sentence.starts_with(']')
                {
                    sentence.remove(0);
                }
                if sentence.len() >= 10 {
                    sentences.push(sentence);
                }
                start = i + 1;
            }
            prev_char_is_digit = char.is_digit(10);
        }

        // Add the last sentence if it's not empty and is 10 characters or longer
        if start < text.len() {
            let mut sentence = text[start..].trim().to_string();
            while sentence.starts_with(',') || sentence.starts_with('(') || sentence.starts_with(')') {
                sentence.remove(0);
            }
            if sentence.len() >= 10 {
                sentences.push(sentence);
            }
        }

        sentences
    }

    /// Splits the input text into groups of sentences.
    ///
    /// A group is defined as a sequence of sentences whose total length exceeds
    /// a specified character minimum. The text is first cleaned and split
    /// into sentences, and then the sentences are grouped together until the
    /// total length of the group exceeds the character minimum. Once the
    /// character minimum is exceeded, a new group is started.
    fn split_into_groups(text: &str, average_group_size: usize) -> Result<Vec<String>, VectorResourceError> {
        let cleaned_text = ParsingHelper::clean_text(text)?;
        let sentences = ParsingHelper::split_into_sentences(&cleaned_text);
        let mut groups = Vec::new();
        let mut current_group = Vec::new();
        let mut current_length = 0;

        for sentence in sentences {
            let sentence_length = sentence.len();

            // A hard 500 character cap to ensure we never go over context length of LLMs.
            if current_length + sentence_length > 500 {
                // If adding the sentence would exceed the limit, push the current group
                // and start a new one. But first, check if the sentence itself is longer
                // than the limit.
                if sentence_length > 500 {
                    // If the sentence is longer than the limit, split it.
                    let (first, second) = sentence.split_at(500);
                    groups.push(current_group.join(" "));
                    groups.push(first.to_string());
                    current_group = vec![second.to_string()];
                    current_length = second.len();
                } else {
                    // If the sentence is not longer than the limit, just start a new group.
                    groups.push(current_group.join(" "));
                    current_group = vec![sentence];
                    current_length = sentence_length;
                }
            } else if current_length + sentence_length > average_group_size {
                // If adding the sentence would exceed the minimum, add it
                // and start a new one.
                current_group.push(sentence);
                groups.push(current_group.join(" "));
                current_group = vec![];
                current_length = 0;
            } else {
                // If adding the sentence would not exceed the limit or the minimum,
                // add it to the current group.
                current_group.push(sentence);
                current_length += sentence_length;
            }
        }

        // Add the last group if it's not empty
        if !current_group.is_empty() {
            groups.push(current_group.join(" "));
        }

        Ok(groups)
    }

    /// Parses a list of strings filled with text into a Document VectorResource,
    /// extracting keywords, and generating embeddings using the supplied
    /// embedding generator.
    ///
    /// Of note, this function assumes you already pre-parsed the text,
    /// performed cleanup, ensured that each String is under the 512 token
    /// limit and is ready to be used to create a DataChunk.
    pub fn parse_text(
        unfiltered_text_list: Vec<String>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        resource_id: &str,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        let mut text_list = unfiltered_text_list;
        text_list.retain(|text| !text.is_empty());

        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, desc, source, resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Parse the pdf into grouped text blocks
        let keywords = ParsingHelper::extract_keywords(&text_list.join(" "), 50);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding_blocking(generator, keywords)?;
        // println!("Generated resource embedding");

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = text_list.len();
        let mut i = 0;
        for text in &text_list {
            let embedding = generator.generate_embedding_default_blocking(text)?;
            embeddings.push(embedding);

            i += 1;
            println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Add the text + embeddings into the doc
        for (i, text) in text_list.iter().enumerate() {
            doc.append_data(text, None, &embeddings[i], parsing_tags);
        }

        Ok(doc)
    }

    /// Parses a PDF from a buffer into a Document VectorResource, automatically
    /// separating sentences + performing text parsing, as well as
    /// generating embeddings using the supplied embedding generator.
    pub fn parse_pdf(
        buffer: &[u8],
        average_chunk_size: u64,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let grouped_text_list = Self::parse_pdf_to_string_list(buffer, average_chunk_size)?;
        let resource_id = Self::generate_data_hash_blake3(buffer);
        Self::parse_text(
            grouped_text_list,
            generator,
            name,
            desc,
            source,
            &resource_id,
            parsing_tags,
        )
    }

    /// Parses the first X pages of a PDF from a buffer and get the top n keywords using RAKE also
    /// it obtains the text to generate a description.
    pub fn parse_pdf_for_keywords_and_description(
        buffer: &[u8],
        number_pages: i32,
        average_chunk_size: u64,
    ) -> Result<SmartPdfOverview, VectorResourceError> {
        let shortened_text = match ParsingHelper::extract_text_from_pdf_buffer(buffer, Some(number_pages)) {
            Ok(text) => text,
            Err(_) => return Err(VectorResourceError::FailedPDFParsing),
        };
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let grouped_text_list = Self::parse_pdf_text_to_string_list(shortened_text.clone(), average_chunk_size)?;
        let resource_id = Self::generate_data_hash_blake3(buffer);

        // TODO: we don't need all of this
        Ok(SmartPdfOverview {
            keywords: vec![],
            description: shortened_text,
            blake3_hash: resource_id,
            grouped_text_list,
        })
    }
}
