/// Converts a string to camelCase format
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    
    // First character should be lowercase in camelCase
    let mut chars = s.chars();
    if let Some(first_char) = chars.next() {
        result.push(first_char.to_ascii_lowercase());
    }
    
    for c in chars {
        if c == '_' || c == '-' || c == ' ' {  // Added space handling
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    
    result
}

/// Converts a string to snake_case format
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_upper = true;
    
    let s = s.replace(' ', "_").replace('-', "_");
    
    for c in s.chars() {
        if c.is_uppercase() {
            if !prev_is_upper && !result.is_empty() {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_is_upper = true;
        } else {
            result.push(c);
            prev_is_upper = false;
        }
    }
    
    result
        .replace("__", "_")
        .trim_matches('_')
        .to_string()
} 