pub fn get_error_context(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let error_line_index = lines.iter().position(|&line| line.contains(")")).unwrap_or(0);

    let previous_line = lines.get(error_line_index.saturating_sub(1)).unwrap_or(&"");
    let error_line = lines.get(error_line_index).unwrap_or(&"");
    let next_line = lines.get(error_line_index + 1).unwrap_or(&"");

    format!("Lines (pre, error and next): {}\n{}\n{}", previous_line, error_line, next_line)
}