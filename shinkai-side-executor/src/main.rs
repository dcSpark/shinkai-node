use image::ImageFormat;
use pdfium_render::prelude::*;

pub fn main() -> Result<(), PdfiumError> {
    // === Static linking ===
    // PDFIUM_STATIC_LIB_PATH="/path-to/shinkai-node/shinkai-side-executor/lib" cargo build
    let pdfium = Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap());

    let document = pdfium.load_pdf_from_file("../files/Shinkai_Protocol_Whitepaper.pdf", None)?;

    let mut image_counter = 0u64;

    for (page_index, page) in document.pages().iter().enumerate() {
        println!("=============== Page {} ===============", page_index + 1);

        println!("-=-=-=- Objects -=-=-=-");
        for object in page.objects().iter() {
            match object.object_type() {
                PdfPageObjectType::Text => {
                    let text_object = object.as_text_object().unwrap();
                    println!(
                        "Text object: [{} {:?}] {:?}",
                        text_object.unscaled_font_size().value,
                        text_object.font().weight()?,
                        text_object.text()
                    );
                }
                PdfPageObjectType::Image => {
                    let image_object = object.as_image_object().unwrap();
                    println!(
                        "Image object: {} x {}",
                        image_object.width()?.value,
                        image_object.height()?.value
                    );

                    image_counter += 1;

                    let image_name = format!("image-{:#03}-page-{}.png", image_counter, page_index + 1);

                    let result = image_object
                        .get_raw_image()?
                        .save_with_format(&image_name, ImageFormat::Png);

                    println!("Image object saved: {:?} {}", result, &image_name);
                }
                PdfPageObjectType::Path => {
                    let path_object = object.as_path_object().unwrap();
                    println!("Path object: {:?}", path_object.bounds());
                }
                object_type => {
                    println!("Object type {:?}", object_type);
                }
            }
        }

        println!("-=-=-=- Annotations -=-=-=-");
        for annotation in page.annotations().iter() {
            println!(
                "Annotation type: {:?}, text: {:?}",
                annotation.annotation_type(),
                page.text().unwrap().for_annotation(&annotation).ok()
            );
        }
    }

    Ok(())
}
