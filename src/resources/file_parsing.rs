use csv::Reader;
use keyphrases::KeyPhraseExtractor;
use mupdf::pdf::PdfDocument;
use regex::Regex;
use sha2::{Digest, Sha256};
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use shinkai_vector_resources::vector_resource::VectorResource;
use std::{io::Cursor, vec};

pub struct FileParser {}

impl FileParser {
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
        let text =
            FileParser::extract_text_from_pdf_buffer(buffer).map_err(|_| VectorResourceError::FailedPDFParsing)?;
        let grouped_text_list = FileParser::split_into_groups(&text, num_characters as usize);

        grouped_text_list
    }

    /// Extracts text from a pdf buffer using MuPDF
    fn extract_text_from_pdf_buffer(buffer: &[u8]) -> Result<String, mupdf::Error> {
        let document = PdfDocument::from_bytes(buffer)?;

        let mut text = String::new();

        for page_number in 0..document.page_count()? {
            let page = document.load_page(page_number)?;
            let page_text = page.to_text()?;
            text.push_str(&page_text);
        }

        Ok(text)
    }

    /// Generates a Sha256 hash of the input data.
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", buffer));
        let result = hasher.finalize();
        format!("{:x}", result)
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
        let cleaned_text = FileParser::clean_text(text)?;
        let sentences = FileParser::split_into_sentences(&cleaned_text);
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
        text_list: Vec<String>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        resource_id: &str,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, desc, source, resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Parse the pdf into grouped text blocks
        let keywords = FileParser::extract_keywords(&text_list.join(" "), 50);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding(generator, keywords)?;
        // println!("Generated resource embedding");

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = text_list.len();
        let mut i = 0;
        for text in &text_list {
            let embedding = generator.generate_embedding_default(text)?;
            embeddings.push(embedding);

            i += 1;
            // println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
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
        source: Option<&str>,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let grouped_text_list = Self::parse_pdf_to_string_list(buffer, average_chunk_size)?;
        let resource_id = Self::generate_data_hash(buffer);
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
}
