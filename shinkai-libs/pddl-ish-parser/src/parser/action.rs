use std::str::FromStr;

use nom::{
    bytes::complete::{tag, take_while1, take_until},
    character::complete::{alpha1, char, multispace0, multispace1, space1},
    combinator::{map, map_res, opt},
    multi::{separated_list0, separated_list1, many1},
    sequence::{delimited, separated_pair, tuple},
    IResult,
};

use super::parameter::Parameter;
use crate::{models::parser_error::ParserError, parser::error_context::get_error_context};
use nom::bytes::complete::take_till1;

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
                FromStr::from_str,
            ),
            char(')'),
        ),
    )(input)
    .map(|(next_input, vec)| (next_input, vec.into_iter().map(String::from).collect()))
}

fn parameter_name(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == '?')(input)
}

pub fn parse_action_nom(input: &str) -> IResult<&str, Action> {
    match parse_action(input) {
        Ok((next_input, action)) => Ok((next_input, action)),
        Err(err) => Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

pub fn parse_parameters(input: &str) -> IResult<&str, Vec<Parameter>> {
    delimited::<_, _, Vec<Parameter>, _, nom::error::Error<_>, _, _, _>(
        tag("("),
        separated_list0(
            multispace1,
            map(
                separated_pair(
                    take_while1(|c: char| c == '?' || c.is_alphanumeric() || c == '_'),
                    tag(" - "),
                    take_while1(|c: char| c.is_alphanumeric() || c == '_'),
                ),
                |(var, typ): (&str, &str)| Parameter {
                    name: format!("{} - {}", var, typ),
                },
            ),
        ),
        tag(")"),
    )(input)
}

pub fn precondition(input: &str) -> IResult<&str, String> {
    let (input, key) = take_while1(|c: char| !c.is_whitespace())(input)?;
    let (input, _) = space1(input)?;
    let (input, param) = take_while1(|c: char| !c.is_whitespace())(input)?;
    let (input, _) = multispace0(input)?;
    let (input, rest) = opt(tuple((take_while1(|c: char| !c.is_whitespace()), multispace0)))(input)?;
    let rest = rest.map(|(r, _)| format!(" {}", r)).unwrap_or_default();
    Ok((input, format!("{} {}{}", key, param, rest)))
}


// TODO: Fix so it separates pre-conditions
pub fn parse_preconditions(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        char('('),
        map_res(
            take_until(")"),
            |s: &str| -> Result<Vec<String>, std::io::Error> { Ok(vec![s.trim().to_string()]) }
        ),
        char(')'),
    )(input)
}

// TODO: Fix so it separates effects
pub fn parse_effects(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        char('('),
        map_res(
            take_until(")"),
            |s: &str| -> Result<Vec<String>, std::io::Error> { Ok(vec![s.trim().to_string()]) }
        ),
        char(')'),
    )(input)
}

// Function to parse an action from a PDDL file
pub fn parse_action(input: &str) -> Result<(&str, Action), ParserError> {
    println!("Parsing input: {}", input);

    let (input, _) = tag("(:action")(input)?;
    eprintln!("Input after tag: {:?}", input);
    let (input, _) = multispace1(input)?;
    eprintln!("Input after multispace1: {:?}", input);
    let (input, name) = take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_')(input)?;
    println!("Parsed name: {}", name);

    let (input, _) = multispace0(input)?;

    eprintln!("Input after multispace0: {:?}", input);
    let (input, _) = tag(":parameters")(input)?;
    let (input, _) = multispace0(input)?;
    eprintln!("Input after tag: {:?}", input);
    let (input, parameters) = parse_parameters(input)?;
    println!("Parsed parameters: {:?}", parameters);

    eprintln!("Input after parameters: {:?}", input);
    let (input, _) = multispace0(input)?;
    let (input, _) = tag(":precondition")(input)?;
    eprintln!("Input after tag: {:?}", input);
    let (input, _) = multispace0(input)?;
    eprintln!("Input after multispace0: {:?}", input);
    let (input, preconditions) = parse_preconditions(input)?;
    eprintln!("Parsed preconditions: {:?}", preconditions);

    let (input, _) = multispace0(input)?;
    eprintln!("Input after multispace0: {:?}", input);
    let (input, _) = tag(":effect")(input)?;
    eprintln!("Input after tag: {:?}", input);
    let (input, _) = multispace0(input)?;
    eprintln!("Input after multispace0: {:?}", input);
    let (input, effects) = parse_effects(input)?;
    eprintln!("Parsed effects: {:?}", effects);

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
