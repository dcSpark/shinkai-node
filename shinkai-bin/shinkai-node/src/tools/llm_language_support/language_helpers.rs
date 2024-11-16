/// Converts a string to camelCase format, ensuring the result is a valid function name
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    // First character should be lowercase in camelCase
    let mut chars = s.chars();
    if let Some(first_char) = chars.next() {
        // Ensure first character is a letter, prepend 'fn' if not
        if !first_char.is_alphabetic() {
            result.push_str("fn");
            capitalize_next = true;
        }
        if !capitalize_next {
            result.push(first_char.to_ascii_lowercase());
        }
    }

    for c in chars {
        if !c.is_alphanumeric() {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }

    // Handle empty string case
    if result.is_empty() {
        result.push_str("fn");
    }

    result
}

/// Converts a string to snake_case format, ensuring the result is a valid function name
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_upper = true;

    // Ensure the name starts with a letter or underscore
    let mut chars = s.chars();
    if let Some(first_char) = chars.next() {
        if !first_char.is_alphabetic() {
            result.push_str("fn_");
        }
        if !result.starts_with("fn_") {
            result.push(first_char.to_ascii_lowercase());
        }
    }

    for c in chars {
        if !c.is_alphanumeric() {
            if !result.ends_with('_') {
                result.push('_');
            }
        } else if c.is_uppercase() {
            if !prev_is_upper && !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_is_upper = true;
        } else {
            result.push(c);
            prev_is_upper = false;
        }
    }

    // Handle empty string case
    if result.is_empty() {
        result.push_str("fn");
    }

    // Clean up multiple underscores and trailing/leading underscores
    result.replace("__", "_").trim_end_matches('_').to_string()
}
