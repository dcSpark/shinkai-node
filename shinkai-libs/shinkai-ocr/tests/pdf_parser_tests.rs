use shinkai_ocr::pdf_parser::PDFParser;

#[tokio::test]
async fn pdf_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::read("../../files/shinkai_intro.pdf")?;
    let pdf_parser = PDFParser::new()?;
    let parsed_pages = pdf_parser.process_pdf_file(file)?;

    assert_eq!(
        parsed_pages.first().unwrap().content.first().unwrap().text,
        "Shinkai Network Manifesto (Early Preview)"
    );

    Ok(())
}
