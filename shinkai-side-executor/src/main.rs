use clap::{command, Parser};
use image::ImageFormat;
use pdfium_render::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, io::BufReader};

#[derive(Debug, Deserialize)]
struct BoundingBox {
    bbox: [u32; 4],
    label: String,
    // confidence: f32,
    // polygon: Vec<[u32; 2]>,
}

#[derive(Debug, Deserialize)]
struct PageLayout {
    bboxes: Vec<BoundingBox>,
    image_bbox: [f32; 4],
    page: u32,
}

#[derive(Debug, Deserialize)]
struct Layout {
    #[serde(flatten)]
    document_layout: HashMap<String, Vec<PageLayout>>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the PDF file
    #[arg(short, long)]
    file: String,

    /// Path to the layout json file
    #[arg(short, long)]
    layout: Option<String>,

    /// Extract pages as images
    #[arg(short, long)]
    extract: bool,
}

pub fn main() -> Result<(), PdfiumError> {
    let args = Args::parse();

    if args.file.is_empty() {
        println!("Please provide a PDF file");
        return Ok(());
    }

    // === Static linking ===
    // PDFIUM_STATIC_LIB_PATH="/path-to/shinkai-node/shinkai-side-executor/lib" cargo build
    let pdfium = Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap());
    let mut document = pdfium.load_pdf_from_file(&args.file, None)?;

    if let Some(layout) = args.layout {
        println!("Parsing with layout.json, annotate detected structures");
        parse_with_layout(&mut document, &layout)?;
    } else if args.extract {
        println!("Extracting pages as images");
        extract_pages(&document)?;
    } else {
        println!("Parsing text, extracting images");
        parse_and_extract(&document)?;
    }

    Ok(())
}

fn parse_with_layout(document: &mut PdfDocument, layout: &str) -> Result<(), PdfiumError> {
    let layout_file = std::fs::File::open(layout).map_err(|err| PdfiumError::IoError(err))?;
    let reader = BufReader::new(layout_file);
    let layout_json: Layout = serde_json::from_reader(reader).map_err(|err| PdfiumError::IoError(err.into()))?;

    let font = document.fonts_mut().helvetica_bold();
    let page_layouts = layout_json.document_layout.iter().next().unwrap().1;

    for (page_index, mut page) in document.pages_mut().iter().enumerate() {
        println!("=============== Page {} ===============", page_index + 1);

        if let Some(page_layout) = page_layouts.iter().find(|layout| layout.page == page_index as u32 + 1) {
            let render_config = PdfRenderConfig::new()
                .set_target_size(page_layout.image_bbox[2] as i32, page_layout.image_bbox[3] as i32);

            for bbox in page_layout.bboxes.iter() {
                let top = page
                    .pixels_to_points(bbox.bbox[0] as i32, bbox.bbox[1] as i32, &render_config)
                    .unwrap();
                let bottom = page
                    .pixels_to_points(bbox.bbox[2] as i32, bbox.bbox[3] as i32, &render_config)
                    .unwrap();

                page.objects_mut().create_path_object_rect(
                    PdfRect::new(bottom.1, top.0, top.1, bottom.0),
                    Some(PdfColor::MAGENTA),
                    Some(PdfPoints::new(6.0)),
                    None,
                )?;

                page.objects_mut().create_path_object_line(
                    top.0,
                    PdfPoints::new(top.1.value + 10.0),
                    PdfPoints::new(top.0.value + (bbox.label.len() * 10) as f32),
                    PdfPoints::new(top.1.value + 10.0),
                    PdfColor::YELLOW,
                    PdfPoints::new(10.0),
                )?;

                page.objects_mut().create_text_object(
                    top.0,
                    PdfPoints::new(top.1.value + 8.0),
                    bbox.label.as_str(),
                    font,
                    PdfPoints::new(12.0),
                )?;
            }
        }
    }

    let _ = document.save_to_file("results/parsed_with_layout.pdf");

    Ok(())
}

fn parse_and_extract(document: &PdfDocument) -> Result<(), PdfiumError> {
    let mut image_counter = 0u64;

    for (page_index, page) in document.pages().iter().enumerate() {
        println!("=============== Page {} ===============", page_index + 1);

        println!("-=-=-=- Objects -=-=-=-");
        for object in page.objects().iter() {
            match object.object_type() {
                PdfPageObjectType::Text => {
                    let text_object = object.as_text_object().unwrap();
                    println!(
                        "Text object: [{} {:?}] {:?} {:?}",
                        text_object.unscaled_font_size().value,
                        text_object.font().weight()?,
                        text_object.bounds(),
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

                    let image_name = format!("results/image-{:#03}-page-{}.png", image_counter, page_index + 1);

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
    }

    Ok(())
}

fn extract_pages(document: &PdfDocument) -> Result<(), PdfiumError> {
    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    for (index, page) in document
        .pages()
        .iter() // ... get an iterator across all pages ...
        .enumerate()
    {
        println!("Exporting page {}", index + 1);
        let result = page
            .render_with_config(&render_config)? // Initializes a bitmap with the given configuration for this page ...
            .as_image() // ... renders it to an Image::DynamicImage ...
            .as_rgba8() // ... sets the correct color space ...
            .ok_or(PdfiumError::ImageError)?
            .save_with_format(format!("results/exported-page-{:#03}.png", index + 1), ImageFormat::Png); // ... and exports it to a JPEG.

        assert!(result.is_ok());
    }

    Ok(())
}
