use crate::models::domain::{Domain, self};
use crate::models::parser_error::ParserError;
use regex::Regex;

use super::action::parse_actions;
use super::domain_type::parse_domain_types;
use super::error_context::get_error_context;
use super::predicate::parse_predicates;

pub fn parse_domain(original_input: &str) -> Result<(String, Domain), ParserError> {
    // Remove comments
    let re = Regex::new(r";.*").unwrap();
    let input_no_comments = re.replace_all(original_input, "").to_string();
    let input = input_no_comments.clone();

    let re = Regex::new(r"\(define \(domain\s+(?P<name>[\w-]+)\)").expect("Failed to compile domain regex");
    let name = re
        .captures(&input_no_comments)
        .and_then(|caps| caps.name("name"))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| ParserError::new("Error parsing domain name".to_string(), get_error_context(input_no_comments.as_str())))?;

    let re =
        Regex::new(r"\(:requirements\s+(?P<requirements>[:\w\s]+)\)").expect("Failed to compile requirements regex");
    let requirements = re
        .captures(&input_no_comments)
        .and_then(|caps| caps.name("requirements"))
        .map(|m| {
            m.as_str()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
        .ok_or_else(|| ParserError::new("Error parsing requirements".to_string(), get_error_context(input_no_comments.as_str())))?;

    let (_, actions) = parse_actions(&input)?;
    // eprintln!("actions: {:?}", actions);
    let (_, predicates) = parse_predicates(&input)?;
    // eprintln!("predicates: {:?}", predicates);
    let (_, domain_types) = parse_domain_types(&input)?;
    // eprintln!("domain_types: {:?}", domain_types);

    Ok((
        input_no_comments,
        Domain {
            name: name.to_string().trim().to_owned(),
            actions,
            requirements,
            predicates,
            types: domain_types,
        },
    ))
}
