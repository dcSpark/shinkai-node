use regex::Regex;
use crate::models::parser_error::ParserError;
use super::error_context::get_error_context;

#[derive(Debug, PartialEq)]
pub struct DomainType {
    pub name: String,
}

pub fn parse_domain_type_line(line: &str) -> Option<DomainType> {
    if !line.is_empty() {
        Some(DomainType {
            name: line.trim().to_string(),
        })
    } else {
        None
    }
}

pub fn parse_domain_types(input: &str) -> Result<(&str, Vec<DomainType>), ParserError> {
    let type_regex = Regex::new(r"\(:types\s((.|\n)*?)\)").unwrap();

    if let Some(captures) = type_regex.captures(input) {
        let types_str = &captures[1];

        let types: Vec<DomainType> = types_str
            .split_whitespace()
            .map(|name| DomainType { name: name.to_string() })
            .collect();

        let next_input = &input[captures.get(0).unwrap().end()..];
        Ok((next_input, types))
    } else {
        Err(ParserError {
            description: "Failed to parse domain types".to_string(),
            code: get_error_context(input),
        })
    }
}