use super::error::AgentError;
use super::execution::job_prompts::{JobPromptGenerator, Prompt};
use super::job_manager::JobManager;
use csv::Reader;
use lazy_static::lazy_static;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::unstructured::unstructured_api::{self, UnstructuredAPI};
use shinkai_vector_resources::unstructured::unstructured_parser::UnstructuredParser;
use shinkai_vector_resources::unstructured::unstructured_types::UnstructuredElement;
use shinkai_vector_resources::{data_tags::DataTag, source::VRSource};
use std::io::Cursor;

impl JobManager {
    /// Given a list of UnstructuredElements generates a description using the Agent's LLM
    pub async fn generate_description(
        elements: &Vec<UnstructuredElement>,
        agent: SerializedAgent,
    ) -> Result<String, AgentError> {
        // TODO: the 2000 should be dynamic depending on the LLM model
        let prompt = ParsingHelper::process_elements_into_description_prompt(&elements, 2000);
        let desc = Some(ParsingHelper::ending_stripper(
            &JobManager::inference_agent_and_extract(agent.clone(), prompt, "answer").await?,
        ));
        eprintln!("LLM Generated File Description: {:?}", desc);
        Ok(desc.unwrap_or_else(|| "".to_string()))
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings, uses LLM to generate desc and improve overall structure quality,
    ///  and returns a finalized BaseVectorResource.
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource_gen_desc(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        parsing_tags: &Vec<DataTag>,
        agent: SerializedAgent,
        max_node_size: u64,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, AgentError> {
        let (_, source, elements) =
            ParsingHelper::parse_file_helper(file_buffer.clone(), name.clone(), unstructured_api).await?;
        let desc = Self::generate_description(&elements, agent).await?;
        ParsingHelper::parse_elements_into_resource(
            elements,
            generator,
            name,
            Some(desc),
            source,
            parsing_tags,
            max_node_size,
        )
        .await
    }
}

pub struct ParsingHelper {}

impl ParsingHelper {
    /// Generates Blake3 hash of the input data.
    fn generate_data_hash_blake3(content: &[u8]) -> String {
        UnstructuredParser::generate_data_hash(content)
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings,  and returns a finalized BaseVectorResource.
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        parsing_tags: &Vec<DataTag>,
        max_node_size: u64,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, AgentError> {
        let (_, source, elements) =
            ParsingHelper::parse_file_helper(file_buffer.clone(), name.clone(), unstructured_api).await?;

        Self::parse_elements_into_resource(elements, generator, name, desc, source, parsing_tags, max_node_size).await
    }

    /// Helper method which keeps core logic related to parsing elements into a BaseVectorResource
    pub async fn parse_elements_into_resource(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        max_node_size: u64,
    ) -> Result<BaseVectorResource, AgentError> {
        let resource = UnstructuredParser::process_elements_into_resource(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            max_node_size,
        )
        .await?;

        resource.as_trait_object().print_all_nodes_exhaustive(None, true, false);

        Ok(resource)
    }

    /// Basic helper method which parses file into needed data for generating a BaseVectorResource
    async fn parse_file_helper(
        file_buffer: Vec<u8>,
        name: String,
        unstructured_api: UnstructuredAPI,
    ) -> Result<(String, VRSource, Vec<UnstructuredElement>), AgentError> {
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let source = VRSource::from_file(&name, &file_buffer)?;
        let elements = unstructured_api.file_request(file_buffer, &name).await?;
        Ok((resource_id, source, elements))
    }

    /// Takes the provided elements and creates a description prompt ready to be used
    /// to inference with an LLM.
    pub fn process_elements_into_description_prompt(elements: &Vec<UnstructuredElement>, max_size: usize) -> Prompt {
        let max_node_size = 300;
        let mut descriptions = Vec::new();
        let mut description = String::new();
        let mut total_size = 0;

        for element in elements {
            let element_text = &element.text;
            if description.len() + element_text.len() > max_node_size {
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
    pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VRError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = reader
            .headers()
            .map_err(|_| VRError::FailedCSVParsing)?
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
    pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VRError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = if header {
            reader
                .headers()
                .map_err(|_| VRError::FailedCSVParsing)?
                .iter()
                .map(String::from)
                .collect::<Vec<String>>()
        } else {
            Vec::new()
        };

        let mut result = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| VRError::FailedCSVParsing)?;
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
}
