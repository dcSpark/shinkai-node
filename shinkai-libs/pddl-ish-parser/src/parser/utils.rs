use crate::models::parser_error::ParserError;

use super::error_context::get_error_context;

pub fn extract_balanced(input: &str, open: char, close: char) -> Result<String, ParserError> {
    let mut balance = 0;
    let mut start = 0;
    let mut end = 0;

    for (i, c) in input.chars().enumerate() {
        if c == open {
            if balance == 0 {
                start = i;
            }
            balance += 1;
        } else if c == close {
            balance -= 1;
            if balance == 0 {
                end = i;
                break;
            }
        }
    }

    if balance != 0 {
        return Err(ParserError::new("Unbalanced parentheses".to_string(), get_error_context(input)));
    }

    Ok(input[start..=end].to_string())
}