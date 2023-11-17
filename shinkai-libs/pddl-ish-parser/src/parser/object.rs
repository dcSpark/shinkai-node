use regex::Regex;

use crate::models::parser_error::ParserError;

use super::error_context::get_error_context;


#[derive(Debug, PartialEq)]
pub struct Object {
    pub name: String,
    pub object_type: String,
}

// Function to parse objects
pub fn parse_object_line(line: &str) -> Option<Object> {
    if let Some(index) = line.rfind(" - ") {
        let (name, object_type) = line.split_at(index);
        Some(Object {
            name: name.trim().to_string(),
            object_type: object_type.replace(" - ", "").trim().to_string(),
        })
    } else {
        None
    }
}


pub fn parse_objects(input: &str) -> Result<(&str, Vec<Object>), ParserError> {
    let object_regex = Regex::new(r"\(:objects\s((.|\n)*?)\)").unwrap();

    if let Some(captures) = object_regex.captures(input) {
        let objects_str = &captures[1];

        let objects: Vec<Object> = objects_str
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter_map(parse_object_line)
            .collect();


        let next_input = &input[captures.get(0).unwrap().end()..];
        Ok((next_input, objects))
    } else {
        Err(ParserError {
            description: "Failed to parse objects".to_string(),
            code: get_error_context(input),
        })
    }
}