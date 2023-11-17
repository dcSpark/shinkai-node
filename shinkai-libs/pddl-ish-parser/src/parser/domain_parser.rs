use crate::models::domain::{Domain, self};
use crate::models::parser_error::ParserError;
use regex::Regex;

use super::action::parse_actions;
use super::domain_type::parse_domain_types;
use super::error_context::get_error_context;
use super::predicate::parse_predicates;

pub fn parse_domain(input: &str) -> Result<(&str, Domain), ParserError> {
    let original_input = input;
    let re = Regex::new(r"\(define \(domain\s+(?P<name>[\w-]+)\)").expect("Failed to compile domain regex");
    let name = re
        .captures(input)
        .and_then(|caps| caps.name("name"))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| ParserError::new("Error parsing domain name".to_string(), get_error_context(input)))?;

    let re =
        Regex::new(r"\(:requirements\s+(?P<requirements>[:\w\s]+)\)").expect("Failed to compile requirements regex");
    let requirements = re
        .captures(input)
        .and_then(|caps| caps.name("requirements"))
        .map(|m| {
            m.as_str()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
        .ok_or_else(|| ParserError::new("Error parsing requirements".to_string(), get_error_context(input)))?;

    let (input, actions) = parse_actions(original_input)?;
    eprintln!("actions: {:?}", actions);
    let (input, predicates) = parse_predicates(original_input)?;
    eprintln!("predicates: {:?}", predicates);
    let (input, domain_types) = parse_domain_types(original_input)?;
    eprintln!("domain_types: {:?}", domain_types);

    Ok((
        input,
        Domain {
            name: name.to_string().trim().to_owned(),
            actions,
            requirements,
            predicates,
            types: domain_types,
        },
    ))
}
