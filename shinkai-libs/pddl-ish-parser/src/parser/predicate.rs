use regex::Regex;

use crate::models::parser_error::ParserError;

use super::{error_context::get_error_context, parameter::Parameter, utils::extract_balanced};

#[derive(Debug, PartialEq, Clone)]
pub struct Predicate {
    pub name: String,
    pub parameters: Vec<Parameter>,
}

pub fn parse_predicate_line(line: &str) -> Option<Predicate> {
    eprintln!("line: {:?}", line);
    let parts: Vec<&str> = line.split_whitespace().collect();

    let name = parts[0].trim_matches('(').to_string();
    let mut parameters = Vec::new();

    let mut i = 1;
    while i < parts.len() {
        if parts[i] == "-" {
            if i + 1 < parts.len() {
                parameters.push(Parameter {
                    name: parts[i - 1].to_string(),
                    param_type: parts[i + 1].trim_matches(')').to_string(),
                });
                i += 2;
            } else {
                return None;
            }
        } else {
            i += 1;
        }
    }

    Some(Predicate { name, parameters })
}

pub fn parse_predicates(input: &str) -> Result<(&str, Vec<Predicate>), ParserError> {
    // Find the start of the predicates block
    let start_index = input
        .find("(:predicates")
        .ok_or(ParserError::new("Could not find '(:predicates'".to_string(), "PREDICATES_NOT_FOUND".to_string()))?;

    // Extract the predicates block
    let predicates_block = extract_balanced(&input[start_index..], '(', ')')?;

    let predicates: Vec<Predicate> = predicates_block
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && line.starts_with("(") && line.ends_with(")"))
        .filter_map(parse_predicate_line)
        .collect();

    let next_input = &input[start_index + predicates_block.len()..];
    Ok((next_input, predicates))
}
