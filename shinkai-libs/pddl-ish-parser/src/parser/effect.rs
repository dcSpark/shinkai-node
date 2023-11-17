use crate::models::parser_error::ParserError;
use super::{error_context::get_error_context, utils::extract_balanced};

pub fn extract_effects(input: &str) -> Result<String, ParserError> {
    let start = input.find(":effect").ok_or_else(|| ParserError::new("Keyword :effect not found".to_string(), get_error_context(input)))? + ":effect".len();
    let block = extract_balanced(&input[start..], '(', ')')?;
    Ok(block.trim().to_string())
}

// TODO: Fix it? It is just returning one string right now
// But we don't use it anywhere in Shinkai
pub fn parse_effects(input: &str) -> Result<Vec<String>, ParserError> {
    let effects = vec![input.to_string()];
    Ok(effects)
}