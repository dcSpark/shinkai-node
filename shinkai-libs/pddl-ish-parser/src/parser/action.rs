use std::str::FromStr;

use nom::{
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alpha1, char, multispace0, multispace1, space1},
    combinator::{map, map_res, opt},
    multi::{many1, separated_list0, separated_list1},
    sequence::{delimited, separated_pair, tuple},
    IResult,
};
use regex::Regex;

use super::{parameter::Parameter, utils::extract_balanced};
use crate::{models::parser_error::ParserError, parser::error_context::get_error_context};
use nom::bytes::complete::take_till1;
use nom::error::ParseError;

#[derive(Debug, PartialEq)]
// Define the Action struct
pub struct Action {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub preconditions: Vec<String>,
    pub effects: Vec<String>,
}

fn extract_name(input: &str) -> Result<String, ParserError> {
    // Define the regex with a named capture group "name"
    let re = Regex::new(r":action\s+(?P<name>[\w-]+)").expect("Failed to compile name regex");
    re.captures(input)
        .and_then(|caps| caps.name("name"))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| ParserError::new("Error parsing name".to_string(), get_error_context(input)))
}

fn extract_parameters(input: &str) -> Result<String, ParserError> {
    let re = Regex::new(r":parameters\s*(?P<parameters>\(.*?\))").expect("Failed to compile parameters regex");
    Ok(re
        .captures(input)
        .and_then(|caps| caps.name("parameters"))
        .map(|m| m.as_str())
        .ok_or_else(|| ParserError::new("Error parsing parameters".to_string(), get_error_context(input)))?
        .to_string())
}

pub fn extract_preconditions(input: &str) -> Result<String, ParserError> {
    let start = input.find(":precondition").ok_or_else(|| ParserError::new("Keyword :precondition not found".to_string(), get_error_context(input)))? + ":precondition".len();
    let block = extract_balanced(&input[start..], '(', ')')?;
    Ok(block.trim().to_string())
}

pub fn extract_effects(input: &str) -> Result<String, ParserError> {
    let start = input.find(":effect").ok_or_else(|| ParserError::new("Keyword :effect not found".to_string(), get_error_context(input)))? + ":effect".len();
    let block = extract_balanced(&input[start..], '(', ')')?;
    Ok(block.trim().to_string())
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

// TODO: Fix it? It is just returning one string right now
// But we don't use it anywhere in Shinkai
pub fn parse_preconditions(input: &str) -> Result<Vec<String>, ParserError> {
    let preconditions = vec![input.to_string()];
    Ok(preconditions)
}

// TODO: Fix it? It is just returning one string right now
// But we don't use it anywhere in Shinkai
pub fn parse_effects(input: &str) -> Result<Vec<String>, ParserError> {
    let effects = vec![input.to_string()];
    Ok(effects)
}

// Function to parse an action from a PDDL file
pub fn parse_actions(input: &str) -> Result<(&str, Vec<Action>), ParserError> {
    if !input.contains("(:action") {
        return Err(ParserError::new("Invalid format".to_string(), get_error_context(input)));
    }
    
    eprintln!("Parsing actions from input: {:?}", input);
    let actions_str = input.split("(:action").skip(1); // Skip the first split as it will be empty
    let mut actions = Vec::new();

    for action_str in actions_str {
        let action_body = format!("(:action{}", action_str); // Add `(:action` back to the start of the action body
        eprintln!("Parsing action body: {:?}", action_body);

        let name = extract_name(&action_body)?;
        eprintln!("Parsed name: {:?}", name);

        let parameters_str = extract_parameters(&action_body)?;
        let preconditions_str = extract_preconditions(&action_body)?;
        let effects_str = extract_effects(&action_body)?;

        let parameters = parse_parameters(&parameters_str)?;
        let preconditions = parse_preconditions(&preconditions_str)?;
        let effects = parse_effects(&effects_str)?;

        actions.push(Action {
            name,
            parameters,
            preconditions,
            effects,
        });

        eprintln!("Parsed action: {:?}", actions.last());
    }

    Ok((input, actions))
}