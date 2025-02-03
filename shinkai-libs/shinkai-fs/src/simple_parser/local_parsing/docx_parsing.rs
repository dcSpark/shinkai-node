use docx_rs::*;
use serde_json::Value;

use crate::{
    shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}
};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_docx_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let docx = match read_docx(&file_buffer) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Warning: Error parsing DOCX file: {:?}", e);
                return Err(ShinkaiFsError::FailedDOCXParsing);
            }
        };

        // Convert document to JSON
        let json_str = docx.json();
        let json: Value = match serde_json::from_str(&json_str) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("Warning: Error parsing DOCX JSON: {:?}", e);
                return Err(ShinkaiFsError::FailedDOCXParsing);
            }
        };

        Self::process_docx_json(json, max_node_text_size)
    }

    /// Extracts text content from a run node in the DOCX JSON structure.
    /// In docx-rs, text content is nested under
    /// run["data"]["children"][]["data"]["text"] where the child node has
    /// "type": "text".
    fn extract_text_from_run(run: &Value) -> String {
        let mut text = String::new();
        // Navigate through the nested structure: run -> data -> children -> text nodes
        if let Some(children) = run
            .get("data")
            .and_then(|d| d.get("children"))
            .and_then(|c| c.as_array())
        {
            for child in children {
                if child.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(content) = child.get("data").and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(content);
                    }
                }
            }
        }
        text
    }

    /// Determines if a paragraph is a heading and its level.
    /// In docx-rs, style information is found under
    /// paragraph["data"]["property"]["style"] Returns (is_heading,
    /// heading_level)
    fn get_heading_info(paragraph: &Value) -> (bool, usize) {
        if let Some(data) = paragraph.get("data") {
            if let Some(property) = data.get("property") {
                if let Some(style) = property.get("style").and_then(|s| s.as_str()) {
                    // Case-insensitive check
                    let s_lower = style.to_lowercase();
                    if s_lower == "title" {
                        return (true, 0);
                    } else if s_lower.starts_with("heading") {
                        // If the last char is '1', '2', '3'...
                        if let Some(c) = style.chars().last() {
                            if let Some(num) = c.to_digit(10) {
                                // The test expects Heading1 => depth=0
                                let heading_level = (num - 1) as usize;
                                return (true, heading_level);
                            }
                        }
                        // If we can't parse the number, treat as heading level=0
                        return (true, 0);
                    }
                }
            }
        }
        (false, 0)
    }

    pub fn process_docx_json(json: Value, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        Self::process_docx_json_internal(json, max_node_text_size)
    }

    fn process_docx_json_internal(json: Value, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let mut text_groups = Vec::new();
        let mut current_heading_depth: usize = 0;

        // Process document content
        if let Some(document) = json.get("document") {
            // Grab the top-level array of paragraphs/tables/etc.
            if let Some(children) = document.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    match child.get("type").and_then(|t| t.as_str()) {
                        Some("paragraph") => {
                            let mut text = String::new();

                            // Get heading information
                            let (is_heading, heading_level) = Self::get_heading_info(child);

                            // Extract text from runs
                            if let Some(data) = child.get("data") {
                                if let Some(children) = data.get("children").and_then(|c| c.as_array()) {
                                    for run in children {
                                        if run.get("type").and_then(|t| t.as_str()) == Some("run") {
                                            let run_text = Self::extract_text_from_run(run);
                                            if !run_text.is_empty() {
                                                if !text.is_empty() {
                                                    text.push(' ');
                                                }
                                                text.push_str(&run_text);
                                            }
                                        }
                                    }
                                }
                            }

                            if text.is_empty() {
                                continue;
                            }

                            // Update current_heading_depth only if this is a heading
                            if is_heading {
                                current_heading_depth = heading_level;
                            }

                            // Push each paragraph as its own text group with the current depth
                            ShinkaiFileParser::push_text_group_by_depth(
                                &mut text_groups,
                                current_heading_depth,
                                text,
                                max_node_text_size,
                                None,
                            );
                        }
                        Some("table") => {
                            let mut table_text = String::new();

                            // Process table rows - structure: table["data"]["rows"] -> row["cells"] ->
                            // cell["children"] -> paragraphs -> runs -> text
                            if let Some(data) = child.get("data") {
                                if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                                    for row in rows {
                                        let mut row_texts = Vec::new();

                                        // Process cells in the row
                                        if let Some(cells) = row.get("cells").and_then(|c| c.as_array()) {
                                            for cell in cells {
                                                let mut cell_text = String::new();

                                                // Extract text from paragraphs in the cell
                                                if let Some(children) = cell.get("children").and_then(|c| c.as_array())
                                                {
                                                    for content in children {
                                                        if content.get("type").and_then(|t| t.as_str())
                                                            == Some("paragraph")
                                                        {
                                                            if let Some(data) = content.get("data") {
                                                                if let Some(runs) =
                                                                    data.get("children").and_then(|c| c.as_array())
                                                                {
                                                                    for run in runs {
                                                                        let run_text = Self::extract_text_from_run(run);
                                                                        if !run_text.is_empty() {
                                                                            if !cell_text.is_empty() {
                                                                                cell_text.push(' ');
                                                                            }
                                                                            cell_text.push_str(&run_text);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                row_texts.push(cell_text);
                                            }
                                        }

                                        // Join cells with tabs and add to table text
                                        if !row_texts.is_empty() {
                                            if !table_text.is_empty() {
                                                table_text.push('\n');
                                            }
                                            table_text.push_str(&row_texts.join("\t"));
                                        }
                                    }
                                }
                            }

                            // Push table content as a text group
                            if !table_text.is_empty() {
                                ShinkaiFileParser::push_text_group_by_depth(
                                    &mut text_groups,
                                    current_heading_depth,
                                    table_text,
                                    max_node_text_size,
                                    None,
                                );
                            }
                        }
                        _ => {
                            // Ignore other node types (e.g. "section",
                            // "bookmarkStart", etc.)
                        }
                    }
                }
            }
        }

        Ok(text_groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_docx_json() -> Result<(), Box<dyn std::error::Error>> {
        let sample_json = r#"{
            "document": {
                "children": [
                    {
                        "type": "paragraph",
                        "data": {
                            "children": [
                                {
                                    "type": "run",
                                    "data": {
                                        "children": [
                                            {
                                                "type": "text",
                                                "data": {
                                                    "text": "PRIVATE & CONFIDENTIAL",
                                                    "preserveSpace": true
                                                }
                                            }
                                        ]
                                    }
                                }
                            ],
                            "property": {
                                "style": "Title"
                            }
                        }
                    },
                    {
                        "type": "paragraph",
                        "data": {
                            "children": [
                                {
                                    "type": "run",
                                    "data": {
                                        "children": [
                                            {
                                                "type": "text",
                                                "data": {
                                                    "text": "Regular paragraph text",
                                                    "preserveSpace": true
                                                }
                                            }
                                        ]
                                    }
                                }
                            ]
                        }
                    }
                ]
            }
        }"#;

        let json: Value = serde_json::from_str(sample_json)?;
        let text_groups = LocalFileParser::process_docx_json(json, 1000)?;

        // Debug print all groups
        println!("\nDebug output of all text groups:");
        for (i, group) in text_groups.iter().enumerate() {
            println!("Group {}: text='{}', metadata={:?}", i, group.text, group.metadata);
        }

        // Basic validation of the results
        assert!(!text_groups.is_empty(), "Should extract at least one text group");

        // Verify the content of text groups
        assert_eq!(text_groups.len(), 2, "Should have extracted two text groups");
        assert!(
            text_groups[0].text.contains("PRIVATE & CONFIDENTIAL"),
            "First group should be the heading"
        );
        assert!(
            text_groups[1].text.contains("Regular paragraph text"),
            "Second group should be the regular paragraph"
        );

        // Print extracted text groups for debugging
        for (i, group) in text_groups.iter().enumerate() {
            println!("Group {}: {:?}", i, group);
        }

        Ok(())
    }
}
