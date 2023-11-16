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

use super::action::{parse_action, parse_action_nom};
use super::error_context::get_error_context;
use crate::models::parser_error::ParserError;
use crate::models::problem::Problem;
use nom::branch::alt;
use nom::error::ParseError;

// Function to parse a PDDL problem
pub fn parse_problem(input: &str) -> Result<(&str, Problem), ParserError> {
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
    let (input, domain) = parse_domain(input)?;
    eprintln!("Domain: {:?}", domain);
    let (input, objects) = parse_objects(input)?;
    eprintln!("Objects: {:?}", objects);
    let (input, init) = parse_init(input)?;
    eprintln!("Init: {:?}", init);
    let (input, goal) = parse_goals(input)?;
    eprintln!("Goals: {:?}", goal);
    let (input, actions) = many0(parse_action_nom)(input)?;

    Ok((
        input,
        Problem {
            name: name.to_string().trim().to_owned(),
            domain,
            objects,
            init,
            goal,
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
fn parse_objects(input: &str) -> Result<(&str, Vec<String>), ParserError> {
    eprintln!("Parsing objects with input: {:?}", input);
    let (next_input, res) = delimited(
        tag("(:objects"),
        many0(preceded(
            multispace1::<&str, ParserError>,
            tuple((take_until(" -"), tag(" -"), alt((take_until("\n"), take_until(")"))))),
        )),
        preceded(multispace0, tag(")")),
    )(input)
    .map_err(|err| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;

    eprintln!("Parsed objects after res: {:?}", res);
    let objects = res
        .into_iter()
        .map(|(name, _, typ)| {
            let object_name = name.trim().replace("-", "_");
            let object_type = typ.trim().trim_end_matches(')').trim();
            format!("{} - {}", object_name, object_type)
        })
        .collect();

    Ok((next_input, objects))
}

// Function to parse initial conditions
fn parse_init(input: &str) -> Result<(&str, Vec<String>), ParserError> {
    let (next_input, res) = delimited(
        tag("(:init"),
        many0(preceded(multispace1::<&str, ParserError>, take_until(")"))),
        tag(")"),
    )(input)
    .map_err(|err| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;

    let init_conditions = res.into_iter().map(|s| s.to_string()).collect();
    Ok((next_input, init_conditions))
}

// Function to parse goals
fn parse_goals(input: &str) -> Result<(&str, Vec<String>), ParserError> {
    let (input, _) = tag("(:goal")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;

    let (input, _) = multispace1::<&str, nom::error::Error<&str>>(input)
        .map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
            description: format!("{}", err),
            code: get_error_context(input),
        })?;

    // Capture everything until the next closing parenthesis that marks the end of the goals section
    let (input, goals) = take_until(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;

    // Include the closing parenthesis in the captured goals string
    let (input, _) = tag(")")(input).map_err(|err: nom::Err<nom::error::Error<&str>>| ParserError {
        description: format!("{}", err),
        code: get_error_context(input),
    })?;

    // Return the entire goals section as a single string inside a Vec
    Ok((input, vec![goals.to_string()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domain() {
        let input = "    (:domain web-processing)\n    (:objects ...";
        let expected = "web-processing".to_string();
        let (_, domain) = parse_domain(input).unwrap();
        assert_eq!(domain, expected);
    }

    #[test]
    fn test_parse_objects() {
        let inputs = [
            r#"(:objects
                website-url - url
                all-hyperlinks - links
                ai-news-links - links
            )"#,
            r#"(:objects
                website-url - url
                all-hyperlinks - links
                ai-news-links - links)"#,
        ];

        let expected = vec![
            "website_url - url".to_string(),
            "all_hyperlinks - links".to_string(),
            "ai_news_links - links".to_string(),
        ];

        for input in &inputs {
            let result = parse_objects(input);
            match result {
                Ok((remaining_input, objects)) => {
                    assert_eq!(objects, expected);
                    assert_eq!(remaining_input, "");
                }
                Err(e) => {
                    panic!("Error parsing objects: {:?}", e);
                }
            }
        }
    }

    // TODO: we will come back to this eventually
    // #[test]
    // fn test_parse_goals_simple() {
    //     let input = r#"(:goal
    //         (and
    //             (all-links-extracted website-url all-hyperlinks)
    //             (relevant-links-found all-hyperlinks ai-news-links)
    //         )
    //     )"#;

    //     let expected = vec![
    //         "(and",
    //         "(all-links-extracted website-url all-hyperlinks)",
    //         "(relevant-links-found all-hyperlinks ai-news-links)",
    //         ")",
    //     ]
    //     .into_iter()
    //     .map(String::from)
    //     .collect::<Vec<String>>();

    //     match parse_goals(input) {
    //         Ok((remaining_input, goals)) => {
    //             assert_eq!(goals, expected);
    //             assert_eq!(remaining_input, "");
    //         }
    //         Err(e) => panic!("Error parsing goals: {:?}", e),
    //     }
    // }
}
