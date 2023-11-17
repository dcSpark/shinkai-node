use regex::Regex;
use crate::models::parser_error::ParserError;
use super::error_context::get_error_context;

#[derive(Debug, PartialEq, Clone)]
pub struct Parameter {
    pub name: String,
    pub param_type: String,
}

pub fn extract_parameters(input: &str) -> Result<String, ParserError> {
    let re = Regex::new(r":parameters\s*(?P<parameters>\(.*?\))").expect("Failed to compile parameters regex");
    Ok(re
        .captures(input)
        .and_then(|caps| caps.name("parameters"))
        .map(|m| m.as_str())
        .ok_or_else(|| ParserError::new("Error parsing parameters".to_string(), get_error_context(input)))?
        .to_string())
}

pub fn parse_parameters(input: &str) -> Result<Vec<Parameter>, ParserError> {
    // Regex pattern to match `?name - type` and `name - type` patterns within the parentheses
    let re = Regex::new(r"\??\s*(\w+)\s*-\s*(\w+)").unwrap();
    let mut parameters = Vec::new();

    for caps in re.captures_iter(input) {
        let name = caps.get(1).unwrap().as_str().to_string();
        let param_type = caps.get(2).unwrap().as_str().to_string();
        parameters.push(Parameter { name, param_type });
    }

    Ok(parameters)
}