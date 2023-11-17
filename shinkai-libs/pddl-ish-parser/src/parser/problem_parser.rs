use nom::bytes::complete::take_until;
use nom::combinator::map;
use nom::multi::many0;
use nom::sequence::tuple;
use nom::{
    bytes::complete::tag,
    character::complete::{multispace0, multispace1},
    sequence::{delimited, preceded},
    IResult,
};

use super::action::{parse_actions};
use super::error_context::get_error_context;
use crate::models::parser_error::ParserError;
use crate::models::problem::Problem;
use crate::parser::object::Object;
use nom::branch::alt;
use nom::error::ParseError;
use regex::Regex;

// Function to parse a PDDL problem
pub fn parse_problem(original_input: &str) -> Result<(&str, Problem), ParserError> {
    let input = original_input;
    let (input, _) = tag("(define (problem ")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    eprintln!("Input: {:?}", input);
    let (input, name) = take_until(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    eprintln!("Name: {:?}", name);
    let (input, _) = tag(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    eprintln!("Input: {:?}", input);
    let (_, domain) = parse_domain(input)?;
    eprintln!("Domain: {:?}", domain);
    let (input, objects) = parse_objects(original_input)?;
    eprintln!("Objects: {:?}", objects);

    let (input, actions) = parse_actions(input)?;
    eprintln!("Actions: {:?}", actions);

    Ok((
        input,
        Problem {
            name: name.to_string().trim().to_owned(),
            domain,
            objects,
            init: vec![],
            goal: vec![],
            actions,
        },
    ))
}

pub fn parse_domain(input: &str) -> Result<(&str, String), ParserError> {
    delimited(
        multispace0::<&str, ParserError>,
        preceded(
            tag("(:domain"),
            delimited(multispace1::<&str, ParserError>, take_until(")"), tag(")")),
        ),
        multispace0::<&str, ParserError>,
    )(input)
    .map(|(next_input, domain)| (next_input, domain.to_string()))
    .map_err(|err| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })
}

// Function to parse objects
fn parse_object_line(line: &str) -> Option<Object> {
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
        eprintln!("Objects string: {:?}", objects_str);

        let objects: Vec<Object> = objects_str
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter_map(parse_object_line)
            .collect();

        eprintln!("Parsed objects: {:?}", objects);

        let next_input = &input[captures.get(0).unwrap().end()..];

        Ok((next_input, objects))
    } else {
        Err(ParserError {
            description: "Failed to parse objects".to_string(),
            code: get_error_context(input),
        })
    }
}