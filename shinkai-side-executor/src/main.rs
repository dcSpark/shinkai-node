use clap::Parser;
use shinkai_side_executor::api;
use std::{net::SocketAddr, str::FromStr};

const DEFAULT_ADDRESS: &str = "0.0.0.0:8090";

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = DEFAULT_ADDRESS)]
    address: String,

    #[arg(short, long, default_value = "400")]
    max_node_text_size: u64,

    #[arg(short, long, value_name = "PDF_FILE")]
    parse_pdf: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if let Some(pdf_file) = args.parse_pdf {
        let text_groups = shinkai_side_executor::cli::parse_pdf_from_file(&pdf_file, args.max_node_text_size)?;
        println!("{}", serde_json::to_string_pretty(&text_groups)?);
    } else {
        let address = SocketAddr::from_str(&args.address).unwrap();
        api::api_handlers::run_api(address).await?;
    }

    Ok(())
}
