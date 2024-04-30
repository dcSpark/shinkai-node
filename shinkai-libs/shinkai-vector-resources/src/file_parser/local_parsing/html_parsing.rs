use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{file_parser::file_parser_types::TextGroup, resource_errors::VRError};

use super::LocalFileParser;

/// If the file provided is an html file, attempt to extract out the core content to improve
/// overall quality of UnstructuredElements returned.
pub fn extract_core_content(file_buffer: Vec<u8>, file_name: &str) -> Vec<u8> {
    if file_name.ends_with(".html") || file_name.ends_with(".htm") {
        let file_content = String::from_utf8_lossy(&file_buffer);
        let document = Html::parse_document(&file_content);

        // If the file is from GitHub, use a specific selector for GitHub's layout
        if file_name.contains("github.com") {
            if let Ok(layout_selector) = Selector::parse(".entry-content") {
                if let Some(layout_element) = document.select(&layout_selector).next() {
                    return layout_element.inner_html().into_bytes();
                }
            }
        } else if file_name.contains("twitter.com") || file_name.contains("x.com") {
            // Selector for Twitter or X.com's layout
            if let Ok(primary_column_selector) = Selector::parse("div[data-testid='primaryColumn']") {
                if let Some(primary_column_element) = document.select(&primary_column_selector).next() {
                    return primary_column_element.inner_html().into_bytes();
                }
            }
        } else if file_name.contains("youtube.com") {
            // Selector for YouTube's layout
            let mut content = String::new();
            if let Ok(above_the_fold_selector) = Selector::parse("#above-the-fold") {
                if let Some(above_the_fold_element) = document.select(&above_the_fold_selector).next() {
                    content += &above_the_fold_element.inner_html();
                }
            }
            if let Ok(comments_selector) = Selector::parse(".ytd-comments") {
                if let Some(comments_element) = document.select(&comments_selector).next() {
                    content += &comments_element.inner_html();
                }
            }
            return content.into_bytes();
        } else {
            // Try to select the 'main', 'article' tag or a class named 'main'
            if let Ok(main_selector) = Selector::parse("main, .main, article") {
                if let Some(main_element) = document.select(&main_selector).next() {
                    return main_element.inner_html().into_bytes();
                }
            }
        }
    }

    file_buffer
}

impl LocalFileParser {
    const IGNORED_ELEMENTS: &'static [&'static str] = &["head", "script", "svg"];
    const HTML_HEADERS: &'static [&'static str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

    pub fn process_html_file(
        file_buffer: Vec<u8>,
        file_name: &str,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, VRError> {
        let extracted_buffer = extract_core_content(file_buffer, file_name);
        let document = Html::parse_fragment(&String::from_utf8_lossy(&extracted_buffer));

        let mut text_groups: Vec<TextGroup> = Vec::new();

        // heading_parents is used to keep track of the depth of the headings
        let mut heading_parents: Vec<usize> = Vec::with_capacity(6);

        // Iterate through HTML elements and text nodes in order
        fn iter_nodes<'a>(
            element: ElementRef<'a>,
            text_groups: &mut Vec<TextGroup>,
            max_node_text_size: u64,
            heading_parents: &mut Vec<usize>,
        ) -> String {
            let mut node_text = "".to_string();

            for node in element.children() {
                match node.value() {
                    scraper::Node::Element(element) => {
                        let el_name = element.name().to_lowercase();

                        if let Some(element) = ElementRef::wrap(node) {
                            // Jump to next node if the element is ignored
                            if LocalFileParser::IGNORED_ELEMENTS.contains(&element.value().name()) {
                                continue;
                            }

                            // Push current text and start a new text group on section elements
                            if el_name == "article" || el_name == "section" {
                                LocalFileParser::push_text_group_by_depth(
                                    text_groups,
                                    heading_parents.len(),
                                    node_text.clone(),
                                    max_node_text_size,
                                );
                                node_text.clear();
                            }

                            // Header elements
                            if LocalFileParser::HTML_HEADERS.contains(&el_name.as_str()) {
                                let heading_level = el_name
                                    .chars()
                                    .last()
                                    .unwrap_or_default()
                                    .to_digit(10)
                                    .unwrap_or_default() as usize;

                                // Adjust heading_parents based on the current heading level
                                // Find the parent and remove previous child headings
                                if let Some(index) = heading_parents
                                    .iter()
                                    .rposition(|&parent_level| parent_level <= heading_level)
                                {
                                    heading_parents.truncate(index + 1);

                                    if heading_parents[index] < heading_level {
                                        heading_parents.push(heading_level);
                                    }
                                } else {
                                    heading_parents.clear();
                                    heading_parents.push(heading_level);
                                }
                            }

                            match el_name.as_str() {
                                "div" | "button" | "label" => {
                                    if node_text.len() > 0 && !node_text.ends_with(char::is_whitespace) {
                                        node_text.push_str(" ");
                                    }
                                }
                                "p" | "br" => {
                                    if !node_text.is_empty() {
                                        node_text.push_str("\n");
                                    }
                                }
                                "img" => {
                                    let alt = element.attr("alt").unwrap_or("");
                                    let src = element.attr("src").unwrap_or("");

                                    if alt.len() > 0 && src.len() > 0 {
                                        node_text.push_str(&format!(" ![{}]({})", alt, src));
                                    }
                                }
                                _ => (),
                            }

                            // Process child nodes
                            let inner_text = iter_nodes(element, text_groups, max_node_text_size, heading_parents);

                            // Process inner text returned from child nodes
                            if inner_text.len() > 0 {
                                match el_name.as_str() {
                                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                                        let heading_depth = if heading_parents.len() > 0 {
                                            heading_parents.len() - 1
                                        } else {
                                            0
                                        };

                                        LocalFileParser::push_text_group_by_depth(
                                            text_groups,
                                            heading_depth,
                                            inner_text.clone(),
                                            max_node_text_size,
                                        );
                                    }
                                    "a" => {
                                        let href = element.attr("href").unwrap_or("");

                                        if href.len() > 0 && !href.starts_with("#") {
                                            node_text.push_str(&format!(" [{}]({})", inner_text, href));
                                        } else {
                                            node_text.push_str(&format!(" {}", inner_text));
                                        }
                                    }
                                    "code" => {
                                        node_text.push_str(&format!("`{}`", inner_text));
                                    }
                                    "li" => {
                                        node_text.push_str(&format!("* {}\n", inner_text));
                                    }
                                    "ol" => {
                                        // replace asterisks with numbers
                                        inner_text.split("\n").enumerate().for_each(|(index, line)| {
                                            if line.len() > 2 {
                                                node_text.push_str(&format!("{}. {}\n", index + 1, &line[2..]));
                                            }
                                        });
                                    }
                                    _ => {
                                        node_text.push_str(&inner_text);
                                    }
                                }
                            }
                        }
                    }
                    scraper::Node::Text(text) => {
                        if text.text.trim().is_empty() {
                            continue;
                        }

                        // remove multiple whitespaces
                        let re = Regex::new(r"\s{2,}").unwrap();
                        let sanitized_text = re.replace_all(&text.text, " ");

                        node_text.push_str(&sanitized_text);
                    }
                    _ => (),
                };
            }

            node_text
        }

        let result_text = iter_nodes(
            document.root_element(),
            &mut text_groups,
            max_node_text_size,
            &mut heading_parents,
        );

        LocalFileParser::push_text_group_by_depth(
            &mut text_groups,
            heading_parents.len(),
            result_text.clone(),
            max_node_text_size,
        );

        Ok(text_groups)
    }
}
