use pdfium_render::prelude::*;
use regex::Regex;
use std::time::Instant;

use crate::image_parser::ImageParser;

pub struct PDFParser {
    image_parser: ImageParser,
    pdfium: Pdfium,
}

pub struct PDFPage {
    pub page_number: usize,
    pub content: Vec<PDFText>,
}

pub struct PDFText {
    pub text: String,
    pub likely_heading: bool,
}

impl PDFParser {
    pub fn new() -> anyhow::Result<Self> {
        println!("Initializing PDFParser...");
        let image_parser = ImageParser::new()?;
        println!("ImageParser initialized.");

        #[cfg(not(feature = "static"))]
        let pdfium = {
            let lib_path = match std::env::var("PDFIUM_DYNAMIC_LIB_PATH").ok() {
                Some(lib_path) => {
                    println!("Using dynamic library path from environment: {}", lib_path);
                    lib_path
                }
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

                    let path = format!("pdfium/{}-{}", os, arch);
                    println!("Using default library path: {}", path);
                    path
                }
            };

            println!("Using library path: {}", Pdfium::pdfium_platform_library_name_at_path(&lib_path).to_string_lossy().to_string());
            let bindings = match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path)) {
                Ok(bindings) => {
                    println!("Successfully bound to library at: {}", lib_path);
                    bindings
                }
                Err(_) => {
                    println!("Failed to bind to library at: {}. Falling back to current directory.", lib_path);
                    Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))?
                }
            };

            Pdfium::new(bindings)
        };

        #[cfg(feature = "static")]
        let pdfium = {
            println!("Using statically linked library.");
            Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap())
        };

        println!("PDFParser initialized successfully.");
        Ok(PDFParser { image_parser, pdfium })
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>) -> anyhow::Result<Vec<PDFPage>> {
        println!("Starting PDF file processing...");
        let start_time = Instant::now();
        let document = self.pdfium.load_pdf_from_byte_vec(file_buffer, None)?;
        println!("PDF file loaded successfully.");

        struct TextPosition {
            #[allow(dead_code)]
            x: f32,
            y: f32,
        }

        struct TextFont {
            font_size: f32,
            font_weight: PdfFontWeight,
        }

        let mut pdf_pages = Vec::new();
        let mut page_text = "".to_owned();
        let mut previous_text_font: Option<TextFont> = None;

        // Process metadata
        let mut metadata_text = "".to_owned();
        for tag in document.metadata().iter() {
            match tag.tag_type() {
                PdfDocumentMetadataTagType::Title => {
                    let title = tag.value();
                    if !title.is_empty() {
                        metadata_text.push_str(&format!("Title: {}\n", title));
                        println!("Found title: {}", title);
                    }
                }
                PdfDocumentMetadataTagType::Author => {
                    let author = tag.value();
                    if !author.is_empty() {
                        metadata_text.push_str(&format!("Author: {}\n", author));
                        println!("Found author: {}", author);
                    }
                }
                PdfDocumentMetadataTagType::Subject => {
                    let subject = tag.value();
                    if !subject.is_empty() {
                        metadata_text.push_str(&format!("Subject: {}\n", subject));
                        println!("Found subject: {}", subject);
                    }
                }
                PdfDocumentMetadataTagType::Keywords => {
                    let keywords = tag.value();
                    if !keywords.is_empty() {
                        metadata_text.push_str(&format!("Keywords: {}\n", keywords));
                        println!("Found keywords: {}", keywords);
                    }
                }
                _ => {}
            }
        }

        if !metadata_text.is_empty() {
            let pdf_text = PDFText {
                text: metadata_text.trim().to_string(),
                likely_heading: true,
            };
            pdf_pages.push(PDFPage {
                page_number: 0,
                content: vec![pdf_text],
            });
            println!("Metadata processed and added to PDF pages.");
        }

        // Process pages
        for (page_index, page) in document.pages().iter().enumerate() {
            let page_start_time = Instant::now();
            println!("Processing page {}...", page_index + 1);
            let mut pdf_texts = Vec::new();
            let mut previous_text_position: Option<TextPosition> = None;

            for object in page.objects().iter() {
                match object.object_type() {
                    PdfPageObjectType::Text => {
                        let text_object = object.as_text_object().unwrap();
                        let text = text_object.text();

                        if text.is_empty() {
                            println!("Found empty text object, skipping.");
                            continue;
                        }

                        let current_text_position = TextPosition {
                            x: text_object.get_translation().0.value,
                            y: text_object.get_translation().1.value,
                        };

                        let current_text_font = TextFont {
                            font_size: text_object.unscaled_font_size().value,
                            font_weight: text_object.font().weight().unwrap_or(PdfFontWeight::Weight100),
                        };

                        let is_bold = match current_text_font.font_weight {
                            PdfFontWeight::Weight500
                            | PdfFontWeight::Weight600
                            | PdfFontWeight::Weight700Bold
                            | PdfFontWeight::Weight800
                            | PdfFontWeight::Weight900 => {
                                println!("Text is bold.");
                                true
                            }
                            PdfFontWeight::Custom(weight) => {
                                println!("Text has custom weight: {}", weight);
                                weight >= 500
                            }
                            _ => false,
                        };

                        let likely_paragraph = if let (Some(previous_text_position), Some(previous_text_font)) =
                            (previous_text_position.as_ref(), previous_text_font.as_ref())
                        {
                            (current_text_position.y < previous_text_position.y
                                && (previous_text_position.y - current_text_position.y)
                                    > previous_text_font.font_size * 1.5)
                                || (previous_text_position.y < current_text_position.y
                                    && (current_text_position.y - previous_text_position.y)
                                        > previous_text_font.font_size * 1.5)
                        } else {
                            false
                        };

                        let likely_heading = previous_text_font
                            .is_some_and(|f| f.font_size < current_text_font.font_size)
                            && current_text_font.font_size > 12.0
                            && is_bold
                            && text.len() > 1;

                        // Same line, append text
                        if previous_text_position.is_some()
                            && current_text_position.y == previous_text_position.as_ref().unwrap().y
                        {
                            println!("Appending text on the same line: {}", text);
                            page_text.push_str(&text);
                        } else if likely_heading {
                            println!("Found likely heading: {}", text);
                            // Save text from previous text objects.
                            if !page_text.is_empty() {
                                let pdf_text = PDFText {
                                    text: Self::normalize_parsed_text(&page_text),
                                    likely_heading: false,
                                };
                                pdf_texts.push(pdf_text);
                                page_text.clear();
                            }

                            // Add heading to the top level
                            let pdf_text = PDFText {
                                text: Self::normalize_parsed_text(&text),
                                likely_heading: true,
                            };
                            pdf_texts.push(pdf_text);
                        }
                        // likely heading or new paragraph
                        else if likely_paragraph {
                            println!("Found likely paragraph: {}", text);
                            // Save text from previous text objects.
                            if !page_text.is_empty() {
                                let pdf_text = PDFText {
                                    text: Self::normalize_parsed_text(&page_text),
                                    likely_heading: false,
                                };
                                pdf_texts.push(pdf_text);
                                page_text.clear();
                            }

                            page_text.push_str(&text);
                        }
                        // add new line
                        else if page_text.is_empty() {
                            println!("Starting new text block: {}", text);
                            page_text.push_str(&text);
                        } else {
                            println!("Adding new line before text: {}", text);
                            page_text.push_str(&format!("\n{}", &text));
                        }

                        previous_text_position = Some(current_text_position);
                        previous_text_font = Some(current_text_font);
                    }
                    PdfPageObjectType::Image => {
                        println!("Processing image object.");
                        // Save text from previous text objects.
                        if !page_text.is_empty() {
                            let pdf_text = PDFText {
                                text: Self::normalize_parsed_text(&page_text),
                                likely_heading: false,
                            };
                            pdf_texts.push(pdf_text);
                            page_text.clear();
                        }

                        let image_object = object.as_image_object().unwrap();

                        // Unwrap the width and height results
                        let (width, height) = (
                            image_object.width().unwrap().value,
                            image_object.height().unwrap().value,
                        );

                        if width > 50.0 && height > 50.0 {
                            println!("Processing image with width: {}, height: {}", width, height);
                            if let Ok(image) = image_object.get_raw_image() {
                                if let Ok(text) = self.image_parser.process_image(image) {
                                    if !text.is_empty() {
                                        let pdf_text = PDFText {
                                            text: Self::normalize_parsed_text(&text),
                                            likely_heading: false,
                                        };
                                        pdf_texts.push(pdf_text);
                                        println!("Extracted text from image: {}", text);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Drop parsed page numbers as text
            if !page_text.is_empty() && page_text != format!("{}", page_index + 1) {
                let pdf_text = PDFText {
                    text: Self::normalize_parsed_text(&page_text),
                    likely_heading: false,
                };
                pdf_texts.push(pdf_text);
                println!("Added text for page number: {}", page_index + 1);
            }

            page_text.clear();

            pdf_pages.push(PDFPage {
                page_number: page_index + 1,
                content: pdf_texts,
            });

            if std::env::var("DEBUG_VRKAI").is_ok() {
                let page_duration = page_start_time.elapsed();
                println!("Page {} parsed in {:?}", page_index + 1, page_duration);
            }
        }

        if !page_text.is_empty() {
            let pdf_text = PDFText {
                text: Self::normalize_parsed_text(&page_text),
                likely_heading: false,
            };
            pdf_pages
                .last_mut()
                .unwrap_or(&mut PDFPage {
                    page_number: 1,
                    content: Vec::new(),
                })
                .content
                .push(pdf_text);
            println!("Final text added to the last page.");
        }

        if std::env::var("DEBUG_VRKAI").is_ok() {
            let total_duration = start_time.elapsed();
            println!("Total PDF parsed in {:?}", total_duration);
        }

        println!("PDF file processing completed.");
        Ok(pdf_pages)
    }

    fn normalize_parsed_text(parsed_text: &str) -> String {
        println!("Normalizing parsed text...");
        let re_whitespaces = Regex::new(r"\s{2,}|\n").unwrap();
        let re_word_breaks = Regex::new(r"\s*").unwrap();

        let normalized_text = re_whitespaces.replace_all(parsed_text, " ");
        let normalized_text = re_word_breaks.replace_all(&normalized_text, "");
        let normalized_text = normalized_text.trim().to_string();

        println!("Parsed text normalized.");
        normalized_text
    }
}
