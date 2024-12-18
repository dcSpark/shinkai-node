use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

/// If the file provided is an html file, attempt to extract out the core content to improve overall quality.
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

            if let Ok(body_selector) = Selector::parse("body") {
                if let Some(body_element) = document.select(&body_selector).next() {
                    return body_element.inner_html().into_bytes();
                }
            }
        }
    }

    file_buffer
}

impl LocalFileParser {
    const IGNORED_ELEMENTS: &'static [&'static str] = &[
        "base", "head", "link", "meta", "noscript", "script", "style", "svg", "template", "title",
    ];
    const HTML_HEADERS: &'static [&'static str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

    pub fn process_html_file(
        file_buffer: Vec<u8>,
        file_name: &str,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, VRError> {
        let extracted_buffer = extract_core_content(file_buffer, file_name);
        let document = Html::parse_fragment(&String::from_utf8_lossy(&extracted_buffer));

        let mut text_groups: Vec<TextGroup> = Vec::new();

        // to keep track of the current parent headings
        let mut heading_parents: Vec<usize> = Vec::with_capacity(6);

        // Parent nodes propagate context to child nodes.
        // Nodes can alter their state and propagate them to their children.
        #[derive(Default)]
        struct HTMLNodeContext {
            is_preformatted: bool, // pre tags
            is_ordered_list: bool, // ol tags
            list_item_start: u64,  // start attribute for ol tags
            list_depth: u64,       // nested lists
        }

        // Iterate through HTML elements and text nodes in order
        fn iter_nodes<'a>(
            element: ElementRef<'a>,
            text_groups: &mut Vec<TextGroup>,
            max_node_text_size: u64,
            heading_parents: &mut Vec<usize>,
            context: HTMLNodeContext,
        ) -> String {
            let mut node_text = "".to_string();
            let mut list_item_index = context.list_item_start;

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
                            if el_name == "article" || el_name == "section" || el_name == "table" || el_name == "hr" {
                                ShinkaiFileParser::push_text_group_by_depth(
                                    text_groups,
                                    heading_parents.len(),
                                    node_text.trim().to_owned(),
                                    max_node_text_size,
                                    None,
                                );
                                node_text.clear();
                            }

                            // Header elements
                            if LocalFileParser::HTML_HEADERS.contains(&el_name.as_str()) {
                                ShinkaiFileParser::push_text_group_by_depth(
                                    text_groups,
                                    heading_parents.len(),
                                    node_text.trim().to_owned(),
                                    max_node_text_size,
                                    None,
                                );
                                node_text.clear();

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
                                "div" | "button" | "label" | "footer" => {
                                    if node_text.len() > 0 && !node_text.ends_with(char::is_whitespace) {
                                        node_text.push_str(" ");
                                    }
                                }
                                "p" | "br" | "blockquote" => {
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
                                "ol" => {
                                    if !node_text.is_empty() && !node_text.ends_with("\n") {
                                        node_text.push_str("\n");
                                    }

                                    let start = element.attr("start").unwrap_or("1");
                                    list_item_index = start.parse::<u64>().unwrap_or(1);
                                }
                                "ul" => {
                                    if !node_text.is_empty() && !node_text.ends_with("\n") {
                                        node_text.push_str("\n");
                                    }
                                    list_item_index = 1;
                                }
                                _ => (),
                            }

                            let list_depth = if el_name == "ol" || el_name == "ul" {
                                context.list_depth + 1
                            } else {
                                context.list_depth
                            };

                            // Process child nodes
                            let inner_text = iter_nodes(
                                element,
                                text_groups,
                                max_node_text_size,
                                heading_parents,
                                HTMLNodeContext {
                                    is_preformatted: context.is_preformatted || el_name == "pre",
                                    is_ordered_list: (context.is_ordered_list || el_name == "ol") && el_name != "ul",
                                    list_item_start: list_item_index,
                                    list_depth,
                                },
                            );

                            // Process inner text returned from child nodes
                            if inner_text.len() > 0 {
                                match el_name.as_str() {
                                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                                        let heading_depth = if heading_parents.len() > 0 {
                                            heading_parents.len() - 1
                                        } else {
                                            0
                                        };

                                        ShinkaiFileParser::push_text_group_by_depth(
                                            text_groups,
                                            heading_depth,
                                            inner_text.trim().to_owned(),
                                            max_node_text_size,
                                            None,
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
                                    "blockquote" => {
                                        inner_text.split("\n").for_each(|line| {
                                            node_text.push_str(&format!("> {}\n", line));
                                        });
                                    }
                                    "code" => {
                                        if context.is_preformatted {
                                            node_text.push_str(&format!("```\n{}\n```\n", inner_text));
                                        } else {
                                            node_text.push_str(&format!("`{}`", inner_text));
                                        }
                                    }
                                    "li" => {
                                        let list_depth = if context.list_depth > 0 { context.list_depth } else { 1 };
                                        let indentation = "\t".repeat((list_depth - 1) as usize);

                                        if !node_text.is_empty() && !node_text.ends_with("\n") {
                                            node_text.push_str("\n");
                                        }

                                        if context.is_ordered_list {
                                            let li_value = element.attr("value").unwrap_or("");
                                            if let Some(value) = li_value.parse::<u64>().ok() {
                                                list_item_index = value;
                                            }

                                            node_text.push_str(&format!(
                                                "{}{}. {}\n",
                                                indentation,
                                                list_item_index,
                                                inner_text.trim()
                                            ));
                                            list_item_index += 1;
                                        } else {
                                            node_text.push_str(&format!("{}* {}\n", indentation, inner_text.trim()));
                                        }
                                    }
                                    // Push table data to a text group
                                    "table" => {
                                        ShinkaiFileParser::push_text_group_by_depth(
                                            text_groups,
                                            heading_parents.len(),
                                            inner_text.trim().to_owned(),
                                            max_node_text_size,
                                            None,
                                        );
                                    }
                                    "caption" => {
                                        node_text.push_str(&format!("{}\n", inner_text.trim()));
                                    }
                                    "tr" => {
                                        let row_text = inner_text.trim();
                                        let row_text = row_text.trim_end_matches(';');
                                        node_text.push_str(&format!("{}\n", row_text));
                                    }
                                    "td" | "th" => {
                                        node_text.push_str(&format!("{}; ", inner_text));
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

                        // Save preformatted text as is, otherwise remove extra whitespaces
                        if context.is_preformatted {
                            node_text.push_str(&text.text);
                        } else {
                            let re = Regex::new(r"\s{2,}|\n").unwrap();
                            let sanitized_text = re.replace_all(&text.text, " ");

                            node_text.push_str(&sanitized_text);
                        }
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
            HTMLNodeContext::default(),
        );

        ShinkaiFileParser::push_text_group_by_depth(
            &mut text_groups,
            heading_parents.len(),
            result_text.trim().to_owned(),
            max_node_text_size,
            None,
        );

        Ok(text_groups)
    }
}
