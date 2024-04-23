use comrak::{
    nodes::{AstNode, ListDelimType, ListType, NodeValue},
    parse_document, Arena, Options,
};

use crate::{file_parser::file_parser_types::TextGroup, resource_errors::VRError};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_md_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let md_string = String::from_utf8(file_buffer).map_err(|_| VRError::FailedJSONParsing)?;

        let arena = Arena::new();
        let root = parse_document(&arena, &md_string, &Options::default());

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

        let mut heading_parents: Vec<usize> = Vec::with_capacity(6);

        iter_nodes(root, &mut |node| match &node.data.borrow().value {
            NodeValue::Heading(ref heading) => {
                processed_node_type = NodeValue::Heading(heading.clone());
            }
            NodeValue::Paragraph => match processed_node_type {
                NodeValue::Item(_) => {
                    return;
                }
                _ => {
                    processed_node_type = NodeValue::Paragraph;

                    Self::push_text_group_by_depth(
                        &mut text_groups,
                        heading_parents.len(),
                        current_text.clone(),
                        max_node_text_size,
                    );
                    current_text = "".to_string();
                }
            },
            NodeValue::Item(ref list_item) => {
                processed_node_type = NodeValue::Item(list_item.clone());
            }

            NodeValue::Text(ref text) => match processed_node_type {
                NodeValue::Heading(ref heading) => {
                    Self::push_text_group_by_depth(
                        &mut text_groups,
                        heading_parents.len(),
                        current_text.clone(),
                        max_node_text_size,
                    );
                    current_text = "".to_string();

                    let level = heading.level as usize;

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
                        ListType::Bullet => format!("{} ", list_item.bullet_char),
                        ListType::Ordered => match list_item.delimiter {
                            ListDelimType::Period => ". ".to_string(),
                            ListDelimType::Paren => ") ".to_string(),
                        },
                    };

                    current_text.push_str(format!("\n{}{} {}", list_item.start, prefix, text).as_str());

                    processed_node_type = NodeValue::Paragraph;
                }
                _ => {}
            },
            NodeValue::Code(ref code) => {
                current_text.push_str(&code.literal);
            }
            NodeValue::CodeBlock(ref code_block) => {
                current_text.push_str(format!("\n{}", code_block.literal).as_str());
            }
            _ => (),
        });

        Self::push_text_group_by_depth(
            &mut text_groups,
            heading_parents.len(),
            current_text.clone(),
            max_node_text_size,
        );

        Ok(text_groups)
    }

    fn push_text_group_by_depth(text_groups: &mut Vec<TextGroup>, depth: usize, text: String, max_node_text_size: u64) {
        if !text.is_empty() {
            let text_group = TextGroup::new(text.clone(), Default::default(), vec![], vec![], None);

            if depth > 0 {
                let mut parent_group = text_groups.last_mut();
                for _ in 1..depth {
                    if let Some(last_group) = parent_group {
                        parent_group = last_group.sub_groups.last_mut();
                    }
                }

                if let Some(last_group) = parent_group {
                    last_group.push_sub_group(text_group);
                } else {
                    text_groups.push(text_group);
                }
            } else {
                text_groups.push(text_group);
            }
        }
    }
}
