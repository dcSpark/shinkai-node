use shinkai_vector_resources::{
    embedding_generator::EmbeddingGenerator,
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup, unstructured_api::UnstructuredAPI},
    source::{DistributionInfo, TextChunkingStrategy, VRSourceReference},
    vector_resource::BaseVectorResource,
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

    pub async fn generate_resource(
        filename: &str,
        file_buffer: Vec<u8>,
        max_node_text_size: u64,
        generator: &dyn EmbeddingGenerator,
    ) -> anyhow::Result<BaseVectorResource> {
        let text_groups = Self::generate_text_groups(filename, file_buffer, max_node_text_size).await?;

        let cleaned_name = ShinkaiFileParser::clean_name(&filename);
        let source = VRSourceReference::from_file(&filename, TextChunkingStrategy::V1)?;

        ShinkaiFileParser::process_groups_into_resource(
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
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}
