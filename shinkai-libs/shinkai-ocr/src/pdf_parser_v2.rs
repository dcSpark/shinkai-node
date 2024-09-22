use pdfium_render::prelude::*;
use regex::Regex;
use std::time::Instant;

use crate::image_parser::ImageParser;

pub struct PDFParser {
    image_parser: ImageParser,
    pdfium: Pdfium,
}

#[derive(Debug)]
pub struct PDFDocument {
    pub metadata: PDFMetadata,
    pub pages: Vec<PDFPage>,
}

#[derive(Debug)]
pub struct PDFMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Debug)]
pub struct PDFPage {
    pub page_number: usize,
    pub elements: Vec<PDFElement>,
}

#[derive(Debug)]
pub struct PDFElement {
    pub element_type: PDFElementType,
    pub metadata: ElementMetadata,
    pub children: Vec<PDFElement>, // Nested elements
}

#[derive(Debug)]
pub enum PDFElementType {
    Text(PDFText),
    Image(PDFImage),
    // Add more types as needed (e.g., Table, Line, etc.)
}

#[derive(Debug)]
pub struct PDFText {
    pub content: String,
    pub likely_heading: bool,
}

#[derive(Debug)]
pub struct PDFImage {
    pub width: f32,
    pub height: f32,
    pub data: Vec<u8>, // Raw image data or a reference to the image
}

#[derive(Debug)]
pub struct ElementMetadata {
    pub page_number: usize,
    pub object_id: usize,
    pub position: (f32, f32), // (x, y) coordinates
    pub font_size: Option<f32>,
    pub font_weight: Option<PdfFontWeight>,
    pub color: Option<(u8, u8, u8)>, // RGB color
    pub italic: bool,
    pub underline: bool,
    // Add more styles as needed
}

impl PDFParser {
    pub fn new() -> anyhow::Result<Self> {
        let image_parser = ImageParser::new()?;

        #[cfg(not(feature = "static"))]
        let pdfium = {
            let lib_path = match std::env::var("PDFIUM_DYNAMIC_LIB_PATH").ok() {
                Some(lib_path) => lib_path,
                None => {
                    #[cfg(target_os = "linux")]
                    let os = "linux";

                    #[cfg(target_os = "macos")]
                    let os = "mac";

                    #[cfg(target_os = "windows")]
                    let os = "win";

                    #[cfg(target_arch = "aarch64")]
                    let arch = "arm64";

                    #[cfg(target_arch = "x86_64")]
                    let arch = "x64";

                    format!("pdfium/{}-{}", os, arch)
                }
            };

            // Look for the dynamic library in the specified path or fall back to the current directory.
            let bindings = match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path)) {
                Ok(bindings) => bindings,
                Err(_) => Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))?,
            };

            Pdfium::new(bindings)
        };

        #[cfg(feature = "static")]
        let pdfium = Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap());

        Ok(PDFParser { image_parser, pdfium })
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>) -> anyhow::Result<PDFDocument> {
        let start_time = Instant::now();
        let document = self.pdfium.load_pdf_from_byte_vec(file_buffer, None)?;

        // Extract metadata
        let metadata = self.extract_metadata(&document);

        let mut pdf_document = PDFDocument {
            metadata,
            pages: Vec::new(),
        };

        // Process each page
        for (page_index, page) in document.pages().iter().enumerate() {
            let page_start_time = Instant::now();
            let mut elements = Vec::new();

            for (object_index, object) in page.objects().iter().enumerate() {
                match object.object_type() {
                    PdfPageObjectType::Text => {
                        if let Ok(element) = self.parse_text_object(&object, page_index + 1, object_index) {
                            elements.push(element);
                        }
                    }
                    PdfPageObjectType::Image => {
                        if let Ok(element) = self.parse_image_object(&object, page_index + 1, object_index) {
                            elements.push(element);
                        }
                    }
                    // Handle other object types here
                    _ => {}
                }
            }

            // Build hierarchical relationships among elements
            let hierarchical_elements = self.build_hierarchy(elements);

            pdf_document.pages.push(PDFPage {
                page_number: page_index + 1,
                elements: hierarchical_elements,
            });

            if std::env::var("DEBUG_VRKAI").is_ok() {
                let page_duration = page_start_time.elapsed();
                println!("Page {} parsed in {:?}", page_index + 1, page_duration);
            }
        }

        if std::env::var("DEBUG_VRKAI").is_ok() {
            let total_duration = start_time.elapsed();
            println!("Total PDF parsed in {:?}", total_duration);
        }

        Ok(pdf_document)
    }

    fn extract_metadata(&self, document: &PdfDocument) -> PDFMetadata {
        let mut metadata = PDFMetadata {
            title: None,
            author: None,
            subject: None,
            keywords: None,
        };

        for tag in document.metadata().iter() {
            match tag.tag_type() {
                PdfDocumentMetadataTagType::Title => {
                    let title = tag.value();
                    if !title.is_empty() {
                        metadata.title = Some(title.to_string());
                    }
                }
                PdfDocumentMetadataTagType::Author => {
                    let author = tag.value();
                    if !author.is_empty() {
                        metadata.author = Some(author.to_string());
                    }
                }
                PdfDocumentMetadataTagType::Subject => {
                    let subject = tag.value();
                    if !subject.is_empty() {
                        metadata.subject = Some(subject.to_string());
                    }
                }
                PdfDocumentMetadataTagType::Keywords => {
                    let keywords = tag.value();
                    if !keywords.is_empty() {
                        metadata.keywords = Some(keywords.to_string());
                    }
                }
                _ => {}
            }
        }

        metadata
    }

    fn parse_text_object(
        &self,
        object: &PdfPageObject,
        page_number: usize,
        object_id: usize,
    ) -> anyhow::Result<PDFElement> {
        let text_object = object
            .as_text_object()
            .ok_or(anyhow::anyhow!("Not a text object"))?;
        let text = text_object.text();

        if text.is_empty() {
            return Err(anyhow::anyhow!("Empty text"));
        }

        let position = (
            text_object.get_translation().0.value,
            text_object.get_translation().1.value,
        );

        let font_size = text_object.unscaled_font_size().value;
        let font_weight = text_object.font().weight();
        let font_weight = match font_weight {
            Ok(weight) => Some(weight),
            Err(_) => None,
        };

        let color = None; // Remove the call to text_color and set color to None

        let italic = false; // Remove the call to is_italic and set italic to false
        let underline = false; // Remove the call to is_underlined and set underline to false

        let likely_heading = self.determine_likely_heading(font_size, &font_weight, &text);

        let pdf_text = PDFText {
            content: Self::normalize_parsed_text(&text),
            likely_heading,
        };

        let metadata = ElementMetadata {
            page_number,
            object_id,
            position,
            font_size: Some(font_size),
            font_weight,
            color,
            italic,
            underline,
        };

        Ok(PDFElement {
            element_type: PDFElementType::Text(pdf_text),
            metadata,
            children: Vec::new(),
        })
    }

    fn parse_image_object(
        &self,
        object: &PdfPageObject,
        page_number: usize,
        object_id: usize,
    ) -> anyhow::Result<PDFElement> {
        let image_object = object.as_image_object().ok_or(anyhow::anyhow!("Not an image object"))?;

        let width = image_object.width().unwrap_or(PdfPoints::ZERO).value;
        let height = image_object.height().unwrap_or(PdfPoints::ZERO).value;

        // Optionally process the image to extract text or other data
        let data = if let Ok(image) = image_object.get_raw_image() {
            image.into_bytes() // Convert DynamicImage to Vec<u8>
        } else {
            Vec::new()
        };

        let pdf_image = PDFImage { width, height, data };

        let position = (
            image_object.get_translation().0.value,
            image_object.get_translation().1.value,
        );

        let color = None; // Images typically don't have a color in the same sense as text
        let italic = false;
        let underline = false;

        let metadata = ElementMetadata {
            page_number,
            object_id,
            position,
            font_size: None,
            font_weight: None,
            color,
            italic,
            underline,
        };

        Ok(PDFElement {
            element_type: PDFElementType::Image(pdf_image),
            metadata,
            children: Vec::new(),
        })
    }

    fn determine_likely_heading(
        &self,
        current_font_size: f32,
        current_font_weight: &Option<PdfFontWeight>,
        text: &str,
    ) -> bool {
        // Implement your logic to determine if the text is likely a heading
        // For example, check if the font size is larger than a threshold and if it's bold
        let is_bold = match current_font_weight {
            Some(PdfFontWeight::Weight500)
            | Some(PdfFontWeight::Weight600)
            | Some(PdfFontWeight::Weight700Bold)
            | Some(PdfFontWeight::Weight800)
            | Some(PdfFontWeight::Weight900) => true,
            Some(PdfFontWeight::Custom(weight)) => *weight >= 500,
            _ => false,
        };

        current_font_size > 12.0 && is_bold && text.len() > 1
    }

    fn build_hierarchy(&self, elements: Vec<PDFElement>) -> Vec<PDFElement> {
        // Implement logic to build a hierarchical tree from flat elements
        // This could involve grouping elements based on positions, font sizes, etc.
        // For simplicity, returning the elements as-is. You can enhance this as needed.
        elements
    }

    fn normalize_parsed_text(parsed_text: &str) -> String {
        let re_whitespaces = Regex::new(r"\s{2,}|\n").unwrap();
        let re_word_breaks = Regex::new(r"\s*").unwrap();

        let normalized_text = re_whitespaces.replace_all(parsed_text, " ");
        let normalized_text = re_word_breaks.replace_all(&normalized_text, "");
        let normalized_text = normalized_text.trim().to_string();

        normalized_text
    }
}
