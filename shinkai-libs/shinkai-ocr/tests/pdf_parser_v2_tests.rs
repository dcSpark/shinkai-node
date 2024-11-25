use image::{Rgba, RgbaImage};
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfPageObjects, PdfRenderConfig};
use shinkai_ocr::pdf_parser_v2::{PDFDocument, PDFElementType, PDFParser};
use shinkai_ocr::pdf_to_md::MarkdownGenerator; // Import the MarkdownGenerator

use std::fs;
use std::io::Write;

#[tokio::test]
async fn pdf_parsing() -> Result<(), Box<dyn std::error::Error>> {
    // Read the PDF file
    let file = fs::read("../../files/shinkai_intro.pdf")?;

    // Initialize the PDF parser
    let pdf_parser = PDFParser::new()?;

    // Process the PDF file
    let parsed_document = pdf_parser.process_pdf_file(file)?;

    // Print the parsed document for debugging
    println!("Parsed document: {:?}", parsed_document);

    // Initialize the Markdown generator with an output directory for images
    let markdown_generator = MarkdownGenerator::new("output_images".to_string())?;

    // Generate markdown from the parsed document
    let markdown = markdown_generator.to_markdown(&parsed_document)?;

    // Print the generated markdown to the console
    println!("Generated Markdown:\n{}", markdown);

    // Search for the text "At several points in the process"
    if let Some((page_number, position)) = find_text_position(&parsed_document, "At several points in the process") {
        println!(
            "Found 'At several points in the process' on page {} at position: {:?}",
            page_number, position
        );

        // Mark the text position in the PDF and save as an image
        pdf_parser.mark_text_position_in_pdf(
            "../../files/shinkai_intro.pdf",
            "../../files/shinkai_intro_marked.png",
            page_number,
            position
        )?;
    } else {
        println!("Text 'At several points in the process' not found");
    }

    // Assert the first page's first element's text content
    if let Some(first_page) = parsed_document.pages.first() {
        if let Some(first_element) = first_page.elements.first() {
            if let PDFElementType::Text(text_element) = &first_element.element_type {
                assert_eq!(text_element.content, "Shinkai Network Manifesto (Early Preview)");
            } else {
                panic!("First element is not a text element");
            }
        } else {
            panic!("No elements found on the first page");
        }
    } else {
        panic!("No pages found in the parsed document");
    }

    Ok(())
}

fn find_text_position(document: &PDFDocument, search_text: &str) -> Option<(usize, (f32, f32))> {
    for page in &document.pages {
        for element in &page.elements {
            if let PDFElementType::Text(text_element) = &element.element_type {
                if text_element.content.contains(search_text) {
                    return Some((page.page_number, element.metadata.position));
                }
            }
        }
    }
    None
}

#[tokio::test]
async fn pdf_parsing_from_local_file() -> Result<(), Box<dyn std::error::Error>> {
    // Path to the local PDF file
    let local_file_path = "../../files/thinkos.pdf";

    // Initialize the PDF parser
    let pdf_parser = PDFParser::new()?;

    // Process the PDF file
    let file = fs::read(local_file_path)?;
    let parsed_document = pdf_parser.process_pdf_file(file)?;

    // Print the parsed document for debugging
    eprintln!("Parsed document: {:?}", parsed_document.metadata);

    // Initialize the Markdown generator with an output directory for images
    let markdown_generator = MarkdownGenerator::new("output_images".to_string())?;

    // Generate markdown from the parsed document
    let markdown = markdown_generator.to_markdown(&parsed_document)?;

    // Print the generated markdown to the console
    eprintln!("Markdown generated");

    // Save the generated markdown to a file called thinkos.md
    let mut file = fs::File::create("../../files/thinkos.md")?;
    file.write_all(markdown.as_bytes())?;

    // Search for the text "At several points in the process"
    if let Some((page_number, position)) = find_text_position(&parsed_document, "At several points in the process") {
        eprintln!(
            "Found 'At several points in the process' on page {} at position: {:?}",
            page_number, position
        );

        // Mark the text position in the PDF and save as an image
        pdf_parser.mark_text_position_in_pdf(
            "../../files/thinkos.pdf",
            "../../files/thinkos_marked.png",
            page_number,
            position
        )?;
    } else {
        println!("Text 'At several points in the process' not found");
    }

    // Assert the first page's first element's text content
    if let Some(first_page) = parsed_document.pages.first() {
        if let Some(first_element) = first_page.elements.first() {
            if let PDFElementType::Text(text_element) = &first_element.element_type {
                assert_eq!(text_element.content, "Think OS");
            } else {
                panic!("First element is not a text element");
            }
        } else {
            panic!("No elements found on the first page");
        }
    } else {
        panic!("No pages found in the parsed document");
    }

    Ok(())
}
