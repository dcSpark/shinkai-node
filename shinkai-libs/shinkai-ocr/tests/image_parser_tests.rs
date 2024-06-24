use shinkai_ocr::image_parser::ImageParser;

#[tokio::test]
async fn image_parsing() -> Result<(), Box<dyn std::error::Error>> {
    ImageParser::check_and_download_dependencies().await?;

    let file = std::fs::read("../../files/product_table.png")?;
    let image_parser = ImageParser::new()?;
    let parsed_text = image_parser.process_image_file(file)?;
    let table = parsed_text.split("\n").collect::<Vec<_>>();

    assert!(table[1].contains("Product"));
    assert!(table[2].contains("Chocolade"));

    Ok(())
}
