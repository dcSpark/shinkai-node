use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator, file_parser::file_parser_types::TextGroup,
    model_type::EmbeddingModelType, vector_resource::VRKai,
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
    #[arg(short, long, value_name = "PDF_FILE")]
    pub file: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    pub output: Option<PathBuf>,

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
    #[arg(short, long, value_name = "FILE")]
    pub file: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    pub output: Option<PathBuf>,

    #[arg(long, default_value = "snowflake-arctic-embed:xs")]
    pub embedding_model: String,

    #[arg(long, default_value = "https://internal.shinkai.com/x-embed-api/")]
    pub embedding_gen_url: String,

    #[arg(long)]
    pub embedding_gen_key: Option<String>,
}

#[derive(Parser)]
pub struct VrkaiViewContentsArgs {
    #[arg(short, long, value_name = "FILE")]
    pub file: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    pub output: Option<PathBuf>,
}

#[derive(Parser)]
pub struct VrpackArgs {
    #[command(subcommand)]
    pub cmd: VrpackCommands,
}

#[derive(Parser)]
pub struct VrpackGenerateFromFilesArgs {
    #[arg(short, long, num_args = 1.., help = "Path to a file. Can be specified multiple times.")]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    pub output: Option<PathBuf>,

    #[arg(long, default_value = "snowflake-arctic-embed:xs")]
    pub embedding_model: String,

    #[arg(long, default_value = "https://internal.shinkai.com/x-embed-api/")]
    pub embedding_gen_url: String,

    #[arg(long)]
    pub embedding_gen_key: Option<String>,

    #[arg(long)]
    pub vrpack_name: Option<String>,
}

#[derive(Parser)]
pub struct VrpackGenerateFromVrkaisArgs {
    #[arg(short, long, num_args = 1.., help = "Path to a VRKai file. Can be specified multiple times.")]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub vrpack_name: Option<String>,
}

#[derive(Subcommand)]
pub enum CliCommands {
    Pdf(PdfArgs),
    Vrkai(VrkaiArgs),
    Vrpack(VrpackArgs),
}

#[derive(Subcommand)]
pub enum PdfCommands {
    ExtractToTextGroups(PdfExtractToTextGroupsArgs),
}

#[derive(Subcommand)]
pub enum VrkaiCommands {
    GenerateFromFile(VrkaiGenerateFromFileArgs),
    ViewContents(VrkaiViewContentsArgs),
}

#[derive(Subcommand)]
pub enum VrpackCommands {
    GenerateFromFiles(VrpackGenerateFromFilesArgs),
    GenerateFromVrkais(VrpackGenerateFromVrkaisArgs),
}

pub struct Cli {}

impl Cli {
    pub async fn run_cli_command(command: CliCommands) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            CliCommands::Pdf(pdf_args) => match pdf_args.cmd {
                PdfCommands::ExtractToTextGroups(pdf_args) => {
                    let text_groups = Cli::pdf_extract_to_text_groups(&pdf_args.file, pdf_args.max_node_text_size)?;

                    if let Some(output_file) = pdf_args.output {
                        std::fs::write(output_file, serde_json::to_string(&text_groups)?)?;
                    } else {
                        println!("{}", serde_json::to_string(&text_groups)?);
                    }
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

                    if let Some(output_file) = vrkai_args.output {
                        std::fs::write(output_file, encoded_vrkai)?;
                    } else {
                        print!("{}", encoded_vrkai);
                    }
                }
                VrkaiCommands::ViewContents(vrkai_args) => {
                    let vrkai = Cli::vrkai_view_contents(&vrkai_args.file).await?;

                    if let Some(output_file) = vrkai_args.output {
                        std::fs::write(output_file, serde_json::to_string(&vrkai)?)?;
                    } else {
                        println!("{}", serde_json::to_string(&vrkai)?);
                    }
                }
            },
            CliCommands::Vrpack(vrpack_args) => match vrpack_args.cmd {
                VrpackCommands::GenerateFromFiles(vrpack_args) => {
                    let encoded_vrpack = Cli::vrpack_generate_from_files(
                        &vrpack_args.file,
                        &vrpack_args.embedding_model,
                        &vrpack_args.embedding_gen_url,
                        vrpack_args.embedding_gen_key,
                        vrpack_args.vrpack_name,
                    )
                    .await?;

                    if let Some(output_file) = vrpack_args.output {
                        std::fs::write(output_file, encoded_vrpack)?;
                    } else {
                        print!("{}", encoded_vrpack);
                    }
                }
                VrpackCommands::GenerateFromVrkais(vrpack_args) => {
                    let encoded_vrpack =
                        Cli::vrpack_generate_from_vrkais(&vrpack_args.file, vrpack_args.vrpack_name).await?;

                    if let Some(output_file) = vrpack_args.output {
                        std::fs::write(output_file, encoded_vrpack)?;
                    } else {
                        print!("{}", encoded_vrpack);
                    }
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

        match FileStreamParser::generate_vrkai(&filename, file_data, &generator).await {
            Ok(vrkai) => {
                let encoded_vrkai = vrkai.encode_as_base64()?;
                Ok(encoded_vrkai)
            }
            Err(e) => Err(e),
        }
    }

    async fn vrkai_view_contents(file_path: &PathBuf) -> anyhow::Result<VRKai> {
        let file_data = std::fs::read(file_path)?;
        let vrkai = VRKai::from_bytes(&file_data)?;

        Ok(vrkai)
    }

    async fn vrpack_generate_from_files(
        file_paths: &Vec<PathBuf>,
        embedding_model: &str,
        embedding_gen_url: &str,
        embedding_gen_key: Option<String>,
        vrpack_name: Option<String>,
    ) -> anyhow::Result<String> {
        let mut files = HashMap::new();
        for file_path in file_paths {
            let file_data = std::fs::read(file_path)?;
            let filename = file_path.file_name().and_then(|name| name.to_str()).unwrap_or("");
            files.insert(filename.to_string(), file_data);
        }

        let generator = RemoteEmbeddingGenerator::new(
            EmbeddingModelType::from_string(&embedding_model)?,
            embedding_gen_url,
            embedding_gen_key,
        );

        match FileStreamParser::generate_vrpack_from_files(files, &generator, vrpack_name.as_deref().unwrap_or(""))
            .await
        {
            Ok(vrpack) => {
                let encoded_vrpack = vrpack.encode_as_base64()?;
                Ok(encoded_vrpack)
            }
            Err(e) => Err(e),
        }
    }

    async fn vrpack_generate_from_vrkais(
        file_paths: &Vec<PathBuf>,
        vrpack_name: Option<String>,
    ) -> anyhow::Result<String> {
        let mut files = Vec::new();
        for file_path in file_paths {
            let file_data = std::fs::read(file_path)?;
            files.push(file_data);
        }

        match FileStreamParser::generate_vrpack_from_vrkais(files, vrpack_name.as_deref().unwrap_or("")).await {
            Ok(vrpack) => {
                let encoded_vrpack = vrpack.encode_as_base64()?;
                Ok(encoded_vrpack)
            }
            Err(e) => Err(e),
        }
    }
}
