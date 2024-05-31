#[cfg(feature = "desktop-only")]
use comrak::{
    nodes::{AstNode, ListDelimType, ListType, NodeValue},
    parse_document, Arena, Options,
};

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

impl LocalFileParser {
    #[cfg(feature = "desktop-only")]
    pub fn process_md_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let md_string = String::from_utf8(file_buffer).map_err(|_| VRError::FailedMDParsing)?;

        let arena = Arena::new();
        let root = parse_document(&arena, &md_string, &Options::default());

        // build up an AST and iterate through nodes in order
        fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
        where
            F: FnMut(&'a AstNode<'a>),
        {
            f(node);
            for c in node.children() {
                iter_nodes(c, f);
            }
        }

        let mut text_groups: Vec<TextGroup> = Vec::new();
        let mut current_text = "".to_string();
        let mut processed_node_type = NodeValue::Document;

        // heading_parents is used to keep track of the depth of the headings
        let mut heading_parents: Vec<usize> = Vec::with_capacity(6);

        iter_nodes(root, &mut |node| match &node.data.borrow().value {
            // Actual text comes in the next text node, set processed_node_type to the proper type
            NodeValue::Heading(ref heading) => {
                processed_node_type = NodeValue::Heading(heading.clone());
            }
            NodeValue::Paragraph => match processed_node_type {
                // paragraph inside a list item
                NodeValue::Item(_) => {
                    return;
                }
                _ => {
                    processed_node_type = NodeValue::Paragraph;

                    if current_text.len() > 0 {
                        current_text.push_str("\n");
                    }
                }
            },
            NodeValue::Item(ref list_item) => {
                processed_node_type = NodeValue::Item(list_item.clone());
            }
            NodeValue::Link(ref link) => {
                processed_node_type = NodeValue::Link(link.clone());
            }
            NodeValue::Image(ref image) => {
                processed_node_type = NodeValue::Image(image.clone());
            }

            NodeValue::Text(ref text) => match processed_node_type {
                NodeValue::Heading(ref heading) => {
                    // Push previous text to a text group
                    Self::push_text_group_by_depth(
                        &mut text_groups,
                        heading_parents.len(),
                        current_text.clone(),
                        max_node_text_size,
                    );
                    current_text = "".to_string();

                    let level = heading.level as usize;

                    // Adjust heading_parents based on the current heading level
                    // Find the parent and remove previous child headings
                    if let Some(index) = heading_parents.iter().rposition(|&parent_level| parent_level <= level) {
                        heading_parents.truncate(index + 1);

                        if heading_parents[index] < level {
                            heading_parents.push(level);
                        }
                    } else {
                        heading_parents.clear();
                        heading_parents.push(level);
                    }

                    let heading_depth = if heading_parents.len() > 0 {
                        heading_parents.len() - 1
                    } else {
                        0
                    };

                    // Create a new text group for the heading
                    // Upcoming content will be added to its subgroups
                    Self::push_text_group_by_depth(
                        &mut text_groups,
                        heading_depth,
                        text.to_string(),
                        max_node_text_size,
                    );
                }
                NodeValue::Paragraph => {
                    current_text.push_str(text);
                }
                NodeValue::Item(ref list_item) => {
                    let prefix = match list_item.list_type {
                        ListType::Bullet => format!("{} ", list_item.bullet_char as char),
                        ListType::Ordered => match list_item.delimiter {
                            ListDelimType::Period => format!("{}. ", list_item.start),
                            ListDelimType::Paren => format!("{}) ", list_item.start),
                        },
                    };

                    current_text.push_str(format!("\n{} {}", prefix, text).as_str());
                    processed_node_type = NodeValue::Paragraph;
                }
                NodeValue::Link(ref link) => {
                    current_text.push_str(format!("[{}]({})", text, link.url).as_str());
                    processed_node_type = NodeValue::Paragraph;
                }
                NodeValue::Image(ref image) => {
                    current_text.push_str(format!("![{}]({})", text, image.url).as_str());
                    processed_node_type = NodeValue::Paragraph;
                }
                _ => (),
            },
            NodeValue::Code(ref code) => {
                let ticks = "`".repeat(code.num_backticks);
                current_text.push_str(format!("{}{}{}", ticks, code.literal, ticks).as_str());
            }
            NodeValue::CodeBlock(ref code_block) => {
                let fence = if code_block.fenced {
                    format!(
                        "{}",
                        (code_block.fence_char as char)
                            .to_string()
                            .repeat(code_block.fence_length)
                    )
                } else {
                    "".to_string()
                };

                current_text
                    .push_str(format!("\n{}{}\n{}{}\n", fence, code_block.info, code_block.literal, fence).as_str());
            }
            NodeValue::HtmlBlock(ref html_block) => {
                current_text.push_str(format!("\n{}", html_block.literal).as_str());
            }
            NodeValue::HtmlInline(ref html_inline) => {
                current_text.push_str(html_inline.as_str());
            }
            NodeValue::LineBreak => {
                current_text.push_str("\n");
            }
            NodeValue::SoftBreak => {
                current_text.push_str("\n");
            }
            // split text groups by ---
            NodeValue::ThematicBreak => {
                Self::push_text_group_by_depth(
                    &mut text_groups,
                    heading_parents.len(),
                    current_text.clone(),
                    max_node_text_size,
                );
                current_text = "".to_string();
            }
            _ => (),
        });

        // Push the last text group
        Self::push_text_group_by_depth(
            &mut text_groups,
            heading_parents.len(),
            current_text.clone(),
            max_node_text_size,
        );

        Ok(text_groups)
    }

    // Creates a new text group and nests it under the last group at the given depth.
    // It splits text groups into chunks if needed and parses metadata in the text.
    pub fn push_text_group_by_depth(
        text_groups: &mut Vec<TextGroup>,
        depth: usize,
        text: String,
        max_node_text_size: u64,
    ) {
        if !text.is_empty() {
            let mut created_text_groups = Vec::new();
            let (parsed_text, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&text);

            if parsed_text.len() as u64 > max_node_text_size {
                let chunks = if parsed_any_metadata {
                    ShinkaiFileParser::split_into_chunks_with_metadata(&text, max_node_text_size as usize)
                } else {
                    ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize)
                };

                for chunk in chunks {
                    let (parsed_chunk, metadata, _) = ShinkaiFileParser::parse_and_extract_metadata(&chunk);
                    created_text_groups.push(TextGroup::new(parsed_chunk, metadata, vec![], None));
                }
            } else {
                created_text_groups.push(TextGroup::new(parsed_text, metadata, vec![], None));
            }

            if depth > 0 {
                let mut parent_group = text_groups.last_mut();
                for _ in 1..depth {
                    if let Some(last_group) = parent_group {
                        parent_group = last_group.sub_groups.last_mut();
                    }
                }

                if let Some(last_group) = parent_group {
                    for text_group in created_text_groups {
                        last_group.push_sub_group(text_group);
                    }
                } else {
                    for text_group in created_text_groups {
                        text_groups.push(text_group);
                    }
                }
            } else {
                for text_group in created_text_groups {
                    text_groups.push(text_group);
                }
            }
        }
    }
}
