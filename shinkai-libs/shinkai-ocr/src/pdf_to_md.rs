use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::pdf_parser_v2::{PDFDocument, PDFElement, PDFElementType, PDFImage, PDFPage};

pub struct MarkdownGenerator {
    image_output_dir: String,
}

impl MarkdownGenerator {
    /// Creates a new MarkdownGenerator with the specified image output directory.
    pub fn new(image_output_dir: String) -> anyhow::Result<Self> {
        // Create the directory if it doesn't exist
        fs::create_dir_all(&image_output_dir)?;
        Ok(MarkdownGenerator { image_output_dir })
    }

    /// Converts a PDFDocument into a Markdown string.
    pub fn to_markdown(&self, document: &PDFDocument) -> anyhow::Result<String> {
        let mut markdown = String::new();

        // Add document metadata as front matter (optional)
        if document.metadata.title.is_some()
            || document.metadata.author.is_some()
            || document.metadata.subject.is_some()
            || document.metadata.keywords.is_some()
        {
            markdown.push_str("---\n");
            if let Some(title) = &document.metadata.title {
                markdown.push_str(&format!("title: \"{}\"\n", title));
            }
            if let Some(author) = &document.metadata.author {
                markdown.push_str(&format!("author: \"{}\"\n", author));
            }
            if let Some(subject) = &document.metadata.subject {
                markdown.push_str(&format!("subject: \"{}\"\n", subject));
            }
            if let Some(keywords) = &document.metadata.keywords {
                markdown.push_str(&format!("keywords: \"{}\"\n", keywords));
            }
            markdown.push_str("---\n\n");
        }

        // Process each page
        for page in &document.pages {
            markdown.push_str(&self.process_page(page)?);
            markdown.push_str("\n\n---\n\n"); // Page separator
        }

        Ok(markdown)
    }

    /// Processes a single PDF page and converts it to Markdown.
    fn process_page(&self, page: &PDFPage) -> anyhow::Result<String> {
        let mut markdown = String::new();

        for element in &page.elements {
            markdown.push_str(&self.process_element(element, 0)?);
            markdown.push('\n');
        }

        Ok(markdown)
    }

    /// Recursively processes a PDF element and its children, converting them to Markdown.
    ///
    /// `indent_level` is used to manage indentation for nested elements.
    fn process_element(&self, element: &PDFElement, indent_level: usize) -> anyhow::Result<String> {
        let mut markdown = String::new();
        let indent = "    ".repeat(indent_level); // 4 spaces per indent level

        match &element.element_type {
            PDFElementType::Text(text) => {
                if text.likely_heading {
                    // Determine heading level based on font size or other criteria
                    // For simplicity, we'll use Markdown's level 2 headings
                    markdown.push_str(&format!("{}## {}\n", indent, text.content));
                } else {
                    // Regular paragraph
                    markdown.push_str(&format!("{}{}\n", indent, text.content));
                }
            }
            PDFElementType::Image(image) => {
                // Save the image and get its path
                let image_filename = self.save_image(image)?;
                // Reference the image in Markdown
                markdown.push_str(&format!("{}![Image]({})\n", indent, image_filename));
            }
            // Handle other element types (e.g., Table, Line) as needed
        }

        // Process children recursively
        for child in &element.children {
            markdown.push_str(&self.process_element(child, indent_level + 1)?);
        }

        Ok(markdown)
    }

    /// Saves an image to the specified output directory and returns the relative path.
    fn save_image(&self, image: &PDFImage) -> anyhow::Result<String> {
        // Generate a unique filename, e.g., image_1.png, image_2.png, etc.
        // For simplicity, use a timestamp or a UUID. Here, we'll use a random UUID.
        use uuid::Uuid;

        let uuid = Uuid::new_v4();
        let filename = format!("image_{}.png", uuid);
        let filepath = Path::new(&self.image_output_dir).join(&filename);

        // Save the image data to the file
        let mut file = File::create(&filepath)?;
        file.write_all(&image.data)?;

        // Return the relative path to be used in Markdown
        Ok(filepath.to_string_lossy().to_string())
    }
}
