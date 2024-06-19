use image::GenericImageView;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use pdfium_render::prelude::*;
use rten::Model;
use std::{io::Write, path::PathBuf};

pub struct PDFParser {
    ocr_engine: OcrEngine,
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

        #[cfg(not(feature = "static"))]
        let pdfium = {
            use std::env;
            let lib_path = env::var("PDFIUM_DYNAMIC_LIB_PATH").unwrap_or("./".to_string());
            Pdfium::new(Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib_path)).unwrap())
        };

        #[cfg(feature = "static")]
        let pdfium = Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap());

        Ok(PDFParser { ocr_engine, pdfium })
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>) -> anyhow::Result<Vec<PDFPage>> {
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

        let mut pdf_pages = Vec::new();
        let mut page_text = "".to_owned();
        let mut previous_text_font: Option<TextFont> = None;

        for (page_index, page) in document.pages().iter().enumerate() {
            let mut pdf_texts = Vec::new();
            let mut previous_text_position: Option<TextPosition> = None;

            for object in page.objects().iter() {
                match object.object_type() {
                    PdfPageObjectType::Text => {
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
                            page_text.push_str(&text);
                        } else if likely_heading {
                            // Save text from previous text objects.
                            if !page_text.is_empty() {
                                let pdf_text = PDFText {
                                    text: page_text.clone(),
                                    likely_heading: false,
                                };
                                pdf_texts.push(pdf_text);

                                page_text.clear();
                            }

                            // Add heading to the top level
                            let pdf_text = PDFText {
                                text: text.clone(),
                                likely_heading: true,
                            };
                            pdf_texts.push(pdf_text);
                        }
                        // likely heading or new paragraph
                        else if likely_paragraph {
                            // Save text from previous text objects.
                            if !page_text.is_empty() {
                                let pdf_text = PDFText {
                                    text: page_text.clone(),
                                    likely_heading: false,
                                };
                                pdf_texts.push(pdf_text);

                                page_text.clear();
                            }

                            page_text.push_str(&text);
                        }
                        // add new line
                        else if page_text.is_empty() {
                            page_text.push_str(&text);
                        } else {
                            page_text.push_str(&format!("\n{}", &text));
                        }

                        previous_text_position = Some(current_text_position);
                        previous_text_font = Some(current_text_font);
                    }
                    PdfPageObjectType::Image => {
                        // Save text from previous text objects.
                        if !page_text.is_empty() {
                            let pdf_text = PDFText {
                                text: page_text.clone(),
                                likely_heading: false,
                            };
                            pdf_texts.push(pdf_text);

                            page_text.clear();
                        }

                        let image_object = object.as_image_object().unwrap();

                        if let Ok(text) = Self::process_image_object(self, image_object) {
                            if !text.is_empty() {
                                let pdf_text = PDFText {
                                    text,
                                    likely_heading: false,
                                };
                                pdf_texts.push(pdf_text);
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Drop parsed page numbers as text
            if !page_text.is_empty() && page_text != format!("{}", page_index + 1) {
                let pdf_text = PDFText {
                    text: page_text.clone(),
                    likely_heading: false,
                };
                pdf_texts.push(pdf_text);
            }

            page_text.clear();

            pdf_pages.push(PDFPage {
                page_number: page_index + 1,
                content: pdf_texts,
            });
        }

        if !page_text.is_empty() {
            let pdf_text = PDFText {
                text: page_text.clone(),
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
        }

        Ok(pdf_pages)
    }

    fn process_image_object(&self, image_object: &PdfPageImageObject) -> anyhow::Result<String> {
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

        Ok(text)
    }

    pub async fn check_and_download_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let _ = std::fs::create_dir("ocrs");

        let ocrs_models_url = "https://ocrs-models.s3-accelerate.amazonaws.com/";
        let detection_model = "text-detection.rten";
        let recognition_model = "text-recognition.rten";

        if !std::path::Path::new(&format!("ocrs/{}", detection_model)).exists() {
            let client = reqwest::Client::new();
            let file_data = client
                .get(format!("{}{}", ocrs_models_url, detection_model))
                .send()
                .await?
                .bytes()
                .await?;

            let mut file = std::fs::File::create(format!("ocrs/{}", detection_model))?;
            file.write_all(&file_data)?;
        }

        if !std::path::Path::new(&format!("ocrs/{}", recognition_model)).exists() {
            let client = reqwest::Client::new();
            let file_data = client
                .get(format!("{}{}", ocrs_models_url, recognition_model))
                .send()
                .await?
                .bytes()
                .await?;

            let mut file = std::fs::File::create(format!("ocrs/{}", recognition_model))?;
            file.write_all(&file_data)?;
        }

        Ok(())
    }
}
