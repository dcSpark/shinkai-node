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

// #[tokio::test]
// Note: needs fixing
async fn pdf_table_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::read("../../files/Shinkai_Table_Test_01.pdf")?;
    let pdf_parser = PDFParser::new()?;
    let parsed_pages = pdf_parser.process_pdf_file(file)?;

    // Print out the content of each page
    for page in &parsed_pages {
        println!("Page Number: {}", page.page_number);
        for text in &page.content {
            println!("Text: {}", text.text);
        }
    }

    assert_eq!(
        parsed_pages.first().unwrap().content.first().unwrap().text,
        "Shinkai Network Manifesto (Early Preview)"
    );

    Ok(())
}
