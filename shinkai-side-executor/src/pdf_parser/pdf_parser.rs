use image::GenericImageView;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use pdfium_render::prelude::*;
use rten::Model;
use shinkai_vector_resources::file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup};
use std::{collections::HashMap, path::PathBuf};

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
            pdfium: Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap()),
        })
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>, max_node_text_size: u64) -> anyhow::Result<Vec<TextGroup>> {
        let document = self.pdfium.load_pdf_from_byte_vec(file_buffer, None)?;

        let mut text_groups = Vec::new();
        let mut page_text = "".to_owned();

        for (page_index, page) in document.pages().iter().enumerate() {
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

                        page_text.push_str(&format!(" {}", &text_object.text()));
                    }
                    PdfPageObjectType::Image => {
                        if !found_image {
                            eprintln!("Processing image objects...");
                            found_image = true;
                        }

                        // Save text from previous text objects.
                        Self::process_text_into_text_groups(
                            &page_text,
                            &mut text_groups,
                            max_node_text_size,
                            page_index + 1,
                        );
                        page_text.clear();

                        let image_object = object.as_image_object().unwrap();
                        Self::process_image_object(
                            self,
                            &image_object,
                            &mut text_groups,
                            max_node_text_size,
                            page_index + 1,
                        )?;
                    }
                    _ => {}
                }
            }

            Self::process_text_into_text_groups(&page_text, &mut text_groups, max_node_text_size, page_index + 1);
            page_text.clear();
        }

        Self::process_text_into_text_groups(
            &page_text,
            &mut text_groups,
            max_node_text_size,
            (document.pages().len()) as usize,
        );

        Ok(text_groups)
    }

    fn process_text_into_text_groups(
        text: &str,
        text_groups: &mut Vec<TextGroup>,
        max_node_text_size: u64,
        page_number: usize,
    ) {
        if text.is_empty() {
            return;
        }

        let (parsed_text, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&text);

        if parsed_text.len() as u64 > max_node_text_size {
            let chunks = if parsed_any_metadata {
                ShinkaiFileParser::split_into_chunks_with_metadata(&text, max_node_text_size as usize)
            } else {
                ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize)
            };

            for chunk in chunks {
                let (parsed_chunk, metadata, _) = ShinkaiFileParser::parse_and_extract_metadata(&chunk);

                let metadata: HashMap<String, String> = {
                    let mut map = metadata.clone();
                    map.insert(
                        ShinkaiFileParser::page_numbers_metadata_key(),
                        format!("[{}]", page_number),
                    );
                    map
                };
                text_groups.push(TextGroup::new(parsed_chunk, metadata, vec![], None));
            }
        } else {
            let metadata: HashMap<String, String> = {
                let mut map = metadata.clone();
                map.insert(
                    ShinkaiFileParser::page_numbers_metadata_key(),
                    format!("[{}]", page_number),
                );
                map
            };
            text_groups.push(TextGroup::new(parsed_text, metadata, vec![], None));
        }
    }

    fn process_image_object(
        &self,
        image_object: &PdfPageImageObject,
        text_groups: &mut Vec<TextGroup>,
        max_node_text_size: u64,
        page_number: usize,
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

        Self::process_text_into_text_groups(&text, text_groups, max_node_text_size, page_number);

        Ok(())
    }
}
