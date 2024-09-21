use base64::decode as base64_decode;
use std::error::Error;

/// Enum to represent the type of data
pub enum DataType {
    Json,
    Base64,
    Normal,
}

/// Struct to encapsulate data processing methods
pub struct DataTypeProcessor;

impl DataTypeProcessor {
    /// Creates a new instance of DataTypeProcessor
    pub fn new() -> Self {
        DataTypeProcessor
    }

    /// Processes input data based on its prefix
    // TODO: pontentially move it to be CoW
    pub fn process_data(&self, input: &str) -> Result<String, Box<dyn Error>> {
        if let Some(stripped) = input.strip_prefix(":::JSON:::") {
            Ok(format!("Processed JSON data: {}", stripped))
        } else if let Some(stripped) = input.strip_prefix(":::BASE64:::") {
            let data = self.parse_base64(stripped)?;
            Ok(format!("Processed Base64 data: {}", data))
        } else {
            Ok(format!("Processed normal string: {}", input))
        }
    }

    /// Processes input data and returns a tuple with the processed string and its type
    pub fn process_data_with_type(&self, input: &str) -> Result<(String, DataType), Box<dyn Error>> {
        if let Some(stripped) = input.strip_prefix(":::JSON:::") {
            Ok((format!("Processed JSON data: {}", stripped), DataType::Json))
        } else if let Some(stripped) = input.strip_prefix(":::BASE64:::") {
            let data = self.parse_base64(stripped)?;
            Ok((format!("Processed Base64 data: {}", data), DataType::Base64))
        } else {
            Ok((format!("Processed normal string: {}", input), DataType::Normal))
        }
    }

    /// Decodes a Base64-encoded string and returns the decoded string
    fn parse_base64(&self, input: &str) -> Result<String, Box<dyn Error>> {
        let decoded_bytes = base64_decode(input)?;
        let decoded_str = String::from_utf8(decoded_bytes)?;
        Ok(decoded_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_string() {
        let processor = DataTypeProcessor::new();
        let input = "Hello, world!";
        let result = processor.process_data(input).unwrap();
        assert_eq!(result, "Processed normal string: Hello, world!");
    }

    #[test]
    fn test_base64_string() {
        let processor = DataTypeProcessor::new();
        let input = ":::BASE64:::SGVsbG8sIHdvcmxkIQ=="; // "Hello, world!"
        let result = processor.process_data(input).unwrap();
        assert_eq!(result, "Processed Base64 data: Hello, world!");
    }

    #[test]
    fn test_json_string() {
        let processor = DataTypeProcessor::new();
        let input = r#":::JSON:::{"message": "Hello, world!"}"#;
        let result = processor.process_data(input).unwrap();
        assert_eq!(result, r#"Processed JSON data: {"message": "Hello, world!"}"#);
    }

    #[test]
    fn test_invalid_base64() {
        let processor = DataTypeProcessor::new();
        let input = ":::BASE64:::invalid base64";
        let result = processor.process_data(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_data_with_type_normal() {
        let processor = DataTypeProcessor::new();
        let input = "Hello, world!";
        let (result, data_type) = processor.process_data_with_type(input).unwrap();
        assert_eq!(result, "Processed normal string: Hello, world!");
        assert!(matches!(data_type, DataType::Normal));
    }

    #[test]
    fn test_process_data_with_type_base64() {
        let processor = DataTypeProcessor::new();
        let input = ":::BASE64:::SGVsbG8sIHdvcmxkIQ=="; // "Hello, world!"
        let (result, data_type) = processor.process_data_with_type(input).unwrap();
        assert_eq!(result, "Processed Base64 data: Hello, world!");
        assert!(matches!(data_type, DataType::Base64));
    }

    #[test]
    fn test_process_data_with_type_json() {
        let processor = DataTypeProcessor::new();
        let input = r#":::JSON:::{"message": "Hello, world!"}"#;
        let (result, data_type) = processor.process_data_with_type(input).unwrap();
        assert_eq!(result, r#"Processed JSON data: {"message": "Hello, world!"}"#);
        assert!(matches!(data_type, DataType::Json));
    }
}
