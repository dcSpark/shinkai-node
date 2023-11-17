use nom::bytes::complete::take_until;
use nom::{
    bytes::complete::tag,
    character::complete::{multispace0, multispace1},
    sequence::{delimited, preceded},
};

use super::action::parse_actions;
use super::error_context::get_error_context;
use super::object::parse_objects;
use crate::models::parser_error::ParserError;
use crate::models::problem::Problem;

// Function to parse a PDDL problem
pub fn parse_problem(original_input: &str) -> Result<(&str, Problem), ParserError> {
    let input = original_input;
    let (input, _) = tag("(define (problem ")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    let (input, name) = take_until(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    let (input, _) = tag(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;
    let (_, domain) = parse_problem_domain(input)?;
    let (input, objects) = parse_objects(original_input)?;

    let (input, actions) = parse_actions(input)?;

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

pub fn parse_problem_domain(input: &str) -> Result<(&str, String), ParserError> {
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
