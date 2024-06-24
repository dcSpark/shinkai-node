use std::io::Write;

use image::{DynamicImage, GenericImageView};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

pub struct ImageParser {
    ocr_engine: OcrEngine,
}

impl ImageParser {
    pub fn new() -> anyhow::Result<Self> {
        let ocrs_path = match std::env::var("NODE_STORAGE_PATH").ok() {
            Some(path) => std::path::PathBuf::from(path).join("ocrs"),
            None => std::path::PathBuf::from("ocrs"),
        };

        // Use the `download-models.sh` script to download the models.
        let detection_model_path = ocrs_path.join("text-detection.rten");
        let rec_model_path = ocrs_path.join("text-recognition.rten");

        let detection_model = Model::load_file(detection_model_path)?;
        let recognition_model = Model::load_file(rec_model_path)?;

        let ocr_engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })?;

        Ok(Self { ocr_engine })
    }

    pub fn process_image_file(&self, file_buffer: Vec<u8>) -> anyhow::Result<String> {
        let image = image::load_from_memory(&file_buffer)?;
        self.process_image(image)
    }

    pub fn process_image(&self, image: DynamicImage) -> anyhow::Result<String> {
        let img_source = ImageSource::from_bytes(image.as_bytes(), image.dimensions())?;

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
        let ocrs_path = match std::env::var("NODE_STORAGE_PATH").ok() {
            Some(path) => std::path::PathBuf::from(path).join("ocrs"),
            None => std::path::PathBuf::from("ocrs"),
        };
        let _ = std::fs::create_dir(&ocrs_path);

        let ocrs_models_url = "https://ocrs-models.s3-accelerate.amazonaws.com/";
        let detection_model = "text-detection.rten";
        let recognition_model = "text-recognition.rten";

        if !ocrs_path.join(detection_model).exists() {
            let client = reqwest::Client::new();
            let file_data = client
                .get(format!("{}{}", ocrs_models_url, detection_model))
                .send()
                .await?
                .bytes()
                .await?;

            let mut file = std::fs::File::create(ocrs_path.join(detection_model))?;
            file.write_all(&file_data)?;
        }

        if !ocrs_path.join(recognition_model).exists() {
            let client = reqwest::Client::new();
            let file_data = client
                .get(format!("{}{}", ocrs_models_url, recognition_model))
                .send()
                .await?
                .bytes()
                .await?;

            let mut file = std::fs::File::create(ocrs_path.join(recognition_model))?;
            file.write_all(&file_data)?;
        }

        Ok(())
    }
}
