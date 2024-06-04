use std::collections::HashMap;

use shinkai_vector_resources::{
    embedding_generator::EmbeddingGenerator,
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup, unstructured_api::UnstructuredAPI},
    source::{DistributionInfo, TextChunkingStrategy, VRSourceReference},
    vector_resource::{VRKai, VRPack, VRPath},
};

use super::PDFParser;

pub struct FileStreamParser {}

impl FileStreamParser {
    pub async fn generate_text_groups(
        filename: &str,
        file_buffer: Vec<u8>,
        max_node_text_size: u64,
    ) -> anyhow::Result<Vec<TextGroup>> {
        let file_extension = filename.split('.').last();
        match file_extension {
            Some("pdf") => {
                let pdf_parser = PDFParser::new()?;

                pdf_parser.process_pdf_file(file_buffer, max_node_text_size)
            }
            _ => {
                let source = VRSourceReference::from_file(&filename, TextChunkingStrategy::V1)?;

                ShinkaiFileParser::process_file_into_text_groups(
                    file_buffer,
                    filename.to_string(),
                    max_node_text_size,
                    source,
                    UnstructuredAPI::new_default(),
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))
            }
        }
    }

    pub async fn generate_vrkai(
        filename: &str,
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
    ) -> anyhow::Result<VRKai> {
        let max_node_text_size = generator.model_type().max_input_token_count() as u64;
        let text_groups = Self::generate_text_groups(filename, file_buffer, max_node_text_size).await?;

        let cleaned_name = ShinkaiFileParser::clean_name(&filename);
        let source = VRSourceReference::from_file(&filename, TextChunkingStrategy::V1)?;

        let resource = ShinkaiFileParser::process_groups_into_resource(
            text_groups,
            generator,
            cleaned_name,
            None,
            source,
            &vec![],
            max_node_text_size,
            DistributionInfo::new_empty(),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()));

        match resource {
            Ok(resource) => Ok(resource.to_vrkai()),
            Err(e) => Err(e),
        }
    }

    pub async fn generate_vrpack(
        files: HashMap<String, Vec<u8>>,
        generator: &dyn EmbeddingGenerator,
        vrpack_name: &str,
    ) -> anyhow::Result<VRPack> {
        let mut all_futures = Vec::new();
        let mut current_batch_futures = Vec::new();

        for (filename, file_buffer) in files {
            let future = async move { FileStreamParser::generate_vrkai(&filename, file_buffer, generator).await };

            current_batch_futures.push(future);

            if current_batch_futures.len() == 10 {
                all_futures.push(current_batch_futures);
                current_batch_futures = Vec::new();
            }
        }

        if current_batch_futures.len() > 0 {
            all_futures.push(current_batch_futures);
        }

        let mut vrkais = Vec::new();

        for future in all_futures {
            for result in futures::future::join_all(future).await {
                let vrkai = result?;
                vrkais.push(vrkai);
            }
        }

        let mut vrpack = VRPack::new_empty(&vrpack_name);
        for vrkai in vrkais {
            vrpack.insert_vrkai(&vrkai, VRPath::root(), true)?;
        }

        Ok(vrpack)
    }
}
