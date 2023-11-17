use crate::parser::error_context::get_error_context;

#[derive(Debug, PartialEq)]
pub struct ParserError {
    pub description: String,
    pub code: String,
}

impl ParserError {
    pub fn new(description: String, code: String) -> Self {
        Self { description, code }
    }
}

impl nom::error::ParseError<&str> for ParserError {
    fn from_error_kind(input: &str, kind: nom::error::ErrorKind) -> Self {
        ParserError {
            description: format!("Parsing error: {:?}. Original error: {:?}", kind, input),
            code: get_error_context(input),
        }
    }

    fn append(_: &str, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl std::convert::From<nom::Err<nom::error::Error<&str>>> for ParserError {
    fn from(err: nom::Err<nom::error::Error<&str>>) -> Self {
        match err {
            nom::Err::Error(e) | nom::Err::Failure(e) => ParserError {
                description: format!("Parsing error: {:?}. Original error: {:?}", e.code, e.input),
                code: get_error_context(e.input),
            },
            nom::Err::Incomplete(_) => ParserError {
                description: "Parsing error: Incomplete".to_string(),
                code: "".to_string(),
            },
        }
    }
}
