use std::str::FromStr;

use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::{alpha1, char, multispace0, multispace1},
    combinator::{map, map_res, opt},
    multi::separated_list0,
    sequence::{delimited, tuple},
    IResult,
};

use crate::{models::parser_error::ParserError, parser::error_context::get_error_context};

use super::parameter::Parameter;

#[derive(Debug, PartialEq)]
// Define the Action struct
pub struct Action {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub preconditions: Vec<String>,
    pub effects: Vec<String>,
}

// Function to parse a list of strings (used for parameters, preconditions, effects)
fn parse_list(input: &str) -> IResult<&str, Vec<String>> {
    separated_list0(
        multispace1,
        delimited::<_, _, String, _, nom::error::Error<_>, _, _, _>(
            char('('),
            map_res(
                take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == ' ' || c == '?'),
                FromStr::from_str
            ),
            char(')')
        )
    )(input)
        .map(|(next_input, vec)| (next_input, vec.into_iter().map(String::from).collect()))
}

fn parameter_name(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)
}

pub fn parse_action_nom(input: &str) -> IResult<&str, Action> {
    match parse_action(input) {
        Ok((next_input, action)) => Ok((next_input, action)),
        Err(err) => Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Tag))),
    }
}

// Function to parse an action from a PDDL file
pub fn parse_action(input: &str) -> Result<(&str, Action), ParserError> {
    println!("Parsing input: {}", input);

    let (input, _) = tag("(:action")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = alpha1(input)?;
    println!("Parsed name: {}", name);

    let (input, _) = multispace0(input)?;

    let (input, parameters) = delimited::<_, _, Vec<Parameter>, _, nom::error::Error<_>, _, _, _>(
        tag("("),
        separated_list0(
            multispace1,
            map(parameter_name, |s: &str| Parameter { name: s.to_string() }),
        ),
        tag(")"),
    )(input)?;
    println!("Parsed parameters: {:?}", parameters);

    let (input, _) = multispace0(input)?;
    let (input, preconditions) = delimited::<_, _, Vec<String>, _, nom::error::Error<_>, _, _, _>(
        tag("("),
        parse_list,
        tag(")"),
    )(input)?;
    
    let (input, _) = multispace0(input)?;
    
    let (input, effects) = delimited::<_, _, Vec<String>, _, nom::error::Error<_>, _, _, _>(
        tag("("),
        parse_list,
        tag(")"),
    )(input)?;

    Ok((
        input,
        Action {
            name: name.to_string(),
            parameters,
            preconditions,
            effects,
        },
    ))
}