use image::GenericImageView;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use pdfium_render::prelude::*;
use rten::Model;
use shinkai_vector_resources::file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup};
use std::path::PathBuf;

pub struct PDFParser {
    ocr_engine: OcrEngine,
    pdfium: Pdfium,
}

impl PDFParser {
    pub fn new() -> anyhow::Result<Self> {
        fn file_path(path: &str) -> PathBuf {
            let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            abs_path.push(path);
            abs_path
        }

        // Use the `download-models.sh` script to download the models.
        let detection_model_path = file_path("ocrs/text-detection.rten");
        let rec_model_path = file_path("ocrs/text-recognition.rten");

        let detection_model = Model::load_file(detection_model_path)?;
        let recognition_model = Model::load_file(rec_model_path)?;

        let ocr_engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })?;

        Ok(PDFParser {
            ocr_engine,

            #[cfg(not(feature = "static"))]
            pdfium: {
                use std::env;
                let lib_path = env::var("PDFIUM_DYNAMIC_LIB_PATH").unwrap_or("./".to_string());
                Pdfium::new(Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path)).unwrap())
            },

            #[cfg(feature = "static")]
            pdfium: Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap()),
        })
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>, max_node_text_size: u64) -> anyhow::Result<Vec<TextGroup>> {
        let document = self.pdfium.load_pdf_from_byte_vec(file_buffer, None)?;

        struct TextPosition {
            #[allow(dead_code)]
            x: f32,
            y: f32,
        }

        struct TextFont {
            font_size: f32,
            font_weight: PdfFontWeight,
        }

        let mut text_groups = Vec::new();
        let mut text_depth: usize = 0;
        let mut page_text = "".to_owned();
        let mut previous_text_font: Option<TextFont> = None;

        for (page_index, page) in document.pages().iter().enumerate() {
            let mut previous_text_position: Option<TextPosition> = None;

            // Debug info
            eprintln!("=============== Page {} ===============", page_index + 1);
            let mut found_text = false;
            let mut found_image = false;

            for object in page.objects().iter() {
                match object.object_type() {
                    PdfPageObjectType::Text => {
                        if !found_text {
                            eprintln!("Processing text objects...");
                            found_text = true;
                        }

                        let text_object = object.as_text_object().unwrap();
                        let text = text_object.text();

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
                            | PdfFontWeight::Weight900 => true,
                            PdfFontWeight::Custom(weight) => weight >= 500,
                            _ => false,
                        };

                        let likely_paragraph = if let (Some(previous_text_position), Some(previous_text_font)) =
                            (previous_text_position.as_ref(), previous_text_font.as_ref())
                        {
                            current_text_position.y < previous_text_position.y
                                && (previous_text_position.y - current_text_position.y)
                                    > previous_text_font.font_size * 1.5
                        } else {
                            false
                        };

                        let likely_heading = (likely_paragraph || previous_text_position.is_none())
                            && previous_text_font.is_none()
                            || previous_text_font.is_some_and(|f| f.font_size < current_text_font.font_size)
                                && current_text_font.font_size > 12.0
                                && is_bold
                                && text.len() > 1;

                        // Same line, append text
                        if previous_text_position.is_some()
                            && current_text_position.y == previous_text_position.as_ref().unwrap().y
                        {
                            page_text.push_str(&format!("{}", &text));
                        } else if likely_heading {
                            // Save text from previous text objects.
                            ShinkaiFileParser::push_text_group_by_depth(
                                &mut text_groups,
                                text_depth,
                                page_text.clone(),
                                max_node_text_size,
                            );
                            page_text.clear();

                            // Add heading to the top level
                            ShinkaiFileParser::push_text_group_by_depth(
                                &mut text_groups,
                                0,
                                text.clone(),
                                max_node_text_size,
                            );

                            text_depth = 1;
                        }
                        // likely heading or new paragraph
                        else if likely_paragraph {
                            // Save text from previous text objects.
                            ShinkaiFileParser::push_text_group_by_depth(
                                &mut text_groups,
                                text_depth,
                                page_text.clone(),
                                max_node_text_size,
                            );
                            page_text.clear();

                            page_text.push_str(&format!("{}", &text));
                        }
                        // add new line
                        else {
                            if page_text.is_empty() {
                                page_text.push_str(&format!("{}", &text));
                            } else {
                                page_text.push_str(&format!("\n{}", &text));
                            }
                        }

                        previous_text_position = Some(current_text_position);
                        previous_text_font = Some(current_text_font);
                    }
                    PdfPageObjectType::Image => {
                        if !found_image {
                            eprintln!("Processing image objects...");
                            found_image = true;
                        }

                        // Save text from previous text objects.
                        ShinkaiFileParser::push_text_group_by_depth(
                            &mut text_groups,
                            text_depth,
                            page_text.clone(),
                            max_node_text_size,
                        );
                        page_text.clear();

                        let image_object = object.as_image_object().unwrap();

                        if let Err(err) = Self::process_image_object(
                            self,
                            &image_object,
                            &mut text_groups,
                            max_node_text_size,
                            text_depth,
                        ) {
                            eprintln!("Error processing image object: {:?}", err);
                            match image_object.get_raw_image() {
                                Ok(img) => {
                                    let current_time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
                                    let _ = std::fs::create_dir("broken_images");
                                    let img_path = format!("broken_images/image_{}.png", current_time);

                                    match img.save_with_format(&img_path, image::ImageFormat::Png) {
                                        Ok(_) => {
                                            eprintln!("Saved image to {}", &img_path);
                                        }
                                        Err(err) => {
                                            eprintln!("Error saving image: {:?}", err);
                                        }
                                    }
                                }
                                Err(err) => {
                                    eprintln!("Error getting raw image: {:?}", err);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Drop parsed page numbers as text
            if page_text != format!("{}", page_index + 1) {
                ShinkaiFileParser::push_text_group_by_depth(
                    &mut text_groups,
                    text_depth,
                    page_text.clone(),
                    max_node_text_size,
                );
            }

            page_text.clear();
        }

        ShinkaiFileParser::push_text_group_by_depth(
            &mut text_groups,
            text_depth,
            page_text.clone(),
            max_node_text_size,
        );

        Ok(text_groups)
    }

    fn process_image_object(
        &self,
        image_object: &PdfPageImageObject,
        text_groups: &mut Vec<TextGroup>,
        max_node_text_size: u64,
        text_depth: usize,
    ) -> anyhow::Result<()> {
        let img = image_object.get_raw_image()?;
        let img_source = ImageSource::from_bytes(img.as_bytes(), img.dimensions())?;

        let ocr_input = self.ocr_engine.prepare_input(img_source)?;

        // Get oriented bounding boxes of text words in input image.
        let word_rects = self.ocr_engine.detect_words(&ocr_input)?;

        // Group words into lines. Each line is represented by a list of word bounding boxes.
        let line_rects = self.ocr_engine.find_text_lines(&ocr_input, &word_rects);

        // Recognize the characters in each line.
        let line_texts = self.ocr_engine.recognize_text(&ocr_input, &line_rects)?;

        let text = line_texts
            .iter()
            .flatten()
            .filter_map(|l| {
                let line = l.to_string();
                if line.len() > 1 {
                    Some(line)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        ShinkaiFileParser::push_text_group_by_depth(text_groups, text_depth, text.clone(), max_node_text_size);

        Ok(())
    }
}
