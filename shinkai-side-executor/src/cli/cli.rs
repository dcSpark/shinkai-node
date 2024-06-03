use std::path::PathBuf;

use clap::{Parser, Subcommand};
use shinkai_vector_resources::{
    embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
    file_parser::file_parser_types::TextGroup,
    model_type::EmbeddingModelType,
};

use crate::file_stream_parser::{FileStreamParser, PDFParser};

const DEFAULT_ADDRESS: &str = "0.0.0.0:8090";

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    #[arg(short, long, default_value = DEFAULT_ADDRESS)]
    pub address: String,

    #[command(subcommand)]
    pub cmd: Option<CliCommands>,
}

#[derive(Parser)]
pub struct PdfArgs {
    #[command(subcommand)]
    pub cmd: PdfCommands,
}

#[derive(Parser)]
pub struct PdfExtractToTextGroupsArgs {
    #[arg(long, value_name = "PDF_FILE")]
    pub file: PathBuf,

    #[arg(short, long, default_value = "400")]
    pub max_node_text_size: u64,
}

#[derive(Parser)]
pub struct VrkaiArgs {
    #[command(subcommand)]
    pub cmd: VrkaiCommands,
}

#[derive(Parser)]
pub struct VrkaiGenerateFromFileArgs {
    #[arg(long, value_name = "FILE")]
    pub file: PathBuf,

    #[arg(long, default_value = "snowflake-arctic-embed:xs")]
    pub embedding_model: String,

    #[arg(long, default_value = "https://internal.shinkai.com/x-embed-api/")]
    pub embedding_gen_url: String,

    #[arg(long)]
    pub embedding_gen_key: Option<String>,
}

#[derive(Subcommand)]
pub enum CliCommands {
    Pdf(PdfArgs),
    Vrkai(VrkaiArgs),
}

#[derive(Subcommand)]
pub enum PdfCommands {
    ExtractToTextGroups(PdfExtractToTextGroupsArgs),
}

#[derive(Subcommand)]
pub enum VrkaiCommands {
    GenerateFromFile(VrkaiGenerateFromFileArgs),
}

pub struct Cli {}

impl Cli {
    pub async fn run_cli_command(command: CliCommands) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            CliCommands::Pdf(pdf_args) => match pdf_args.cmd {
                PdfCommands::ExtractToTextGroups(pdf_args) => {
                    let text_groups = Cli::pdf_extract_to_text_groups(&pdf_args.file, pdf_args.max_node_text_size)?;
                    println!("{}", serde_json::to_string_pretty(&text_groups)?);
                }
            },
            CliCommands::Vrkai(vrkai_args) => match vrkai_args.cmd {
                VrkaiCommands::GenerateFromFile(vrkai_args) => {
                    let encoded_vrkai = Cli::vrkai_generate_from_file(
                        &vrkai_args.file,
                        &vrkai_args.embedding_model,
                        &vrkai_args.embedding_gen_url,
                        vrkai_args.embedding_gen_key,
                    )
                    .await?;
                    println!("{}", encoded_vrkai);
                }
            },
        }

        Ok(())
    }

    fn pdf_extract_to_text_groups(file_path: &PathBuf, max_node_text_size: u64) -> anyhow::Result<Vec<TextGroup>> {
        let pdf_parser = PDFParser::new()?;
        let file_data = std::fs::read(file_path)?;

        pdf_parser.process_pdf_file(file_data, max_node_text_size)
    }

    async fn vrkai_generate_from_file(
        file_path: &PathBuf,
        embedding_model: &str,
        embedding_gen_url: &str,
        embedding_gen_key: Option<String>,
    ) -> anyhow::Result<String> {
        let file_data = std::fs::read(file_path)?;
        let filename = file_path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        let generator = RemoteEmbeddingGenerator::new(
            EmbeddingModelType::from_string(&embedding_model)?,
            embedding_gen_url,
            embedding_gen_key,
        );

        match FileStreamParser::generate_vrkai(
            &filename,
            file_data,
            generator.model_type().max_input_token_count() as u64,
            &generator,
        )
        .await
        {
            Ok(vrkai) => {
                let encoded_vrkai = vrkai.encode_as_base64()?;
                Ok(encoded_vrkai)
            }
            Err(e) => Err(e),
        }
    }
}
