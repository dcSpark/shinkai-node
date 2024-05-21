use std::env;

use shinkai_side_executor::pdf_parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if let Some(arg) = args.get(1) {
        let file_buffer = std::fs::read(arg)?;
        let pdf_parser = pdf_parser::PDFParser::new();
        let text_groups = pdf_parser.process_pdf_file(file_buffer, 4096)?;

        for text_group in text_groups {
            println!("Text: {}", text_group.text);
            println!("Metadata: {:?}", text_group.metadata);
            println!();
        }
    } else {
        println!("Usage: {} file.pdf", args[0]);
    }

    Ok(())
}
