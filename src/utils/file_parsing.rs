use csv::Reader;
use pdf_extract;
use std::error::Error;
use std::io::Cursor;

/// Parse CSV data from a buffer.
///
/// # Arguments
///
/// * `buffer` - A byte slice containing the CSV data.
///
/// # Returns
///
/// A `Result` containing a `Vec<Vec<String>>`. Each inner `Vec<String>`
/// represents a row in the CSV, and contains the column values for that row. If
/// an error occurs while parsing the CSV data, the `Result` will contain an
/// `Error`.
fn parse_csv(buffer: &[u8]) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let mut reader = Reader::from_reader(Cursor::new(buffer));
    let mut result = Vec::new();

    for record in reader.records() {
        let record = record?;
        let row: Vec<String> = record.iter().map(String::from).collect();
        result.push(row);
    }

    Ok(result)
}

/// Parse text from a PDF from a buffer.
///
/// # Arguments
///
/// * `buffer` - A byte slice containing the PDF data.
///
/// # Returns
///
/// A `Result` containing a `String` of the extracted text from the PDF. If an
/// error occurs while parsing the PDF data, the `Result` will contain an
/// `Error`.
fn parse_pdf(buffer: &[u8]) -> Result<String, Box<dyn Error>> {
    let text = pdf_extract::extract_text_from_mem(buffer)?;

    Ok(text)
}

/// Parse CSV data from a file.
///
/// # Arguments
///
/// * `file_path` - A string slice representing the file path of the CSV file.
///
/// # Returns
///
/// A `Result` containing a `Vec<Vec<String>>`. Each inner `Vec<String>`
/// represents a row in the CSV, and contains the column values for that row. If
/// an error occurs while parsing the CSV data, the `Result` will contain an
/// `Error`.
fn parse_csv_from_path(file_path: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let mut reader = Reader::from_path(file_path)?;
    let mut result = Vec::new();

    for record in reader.records() {
        let record = record?;
        let row: Vec<String> = record.iter().map(String::from).collect();
        result.push(row);
    }

    Ok(result)
}

/// Parse text from a PDF from a file.
///
/// # Arguments
///
/// * `file_path` - A string slice representing the file path of the PDF file.
///
/// # Returns
///
/// A `Result` containing a `String` of the extracted text from the PDF. If an
/// error occurs while parsing the PDF data, the `Result` will contain an
/// `Error`.
fn parse_pdf_from_path(file_path: &str) -> Result<String, Box<dyn Error>> {
    let bytes = std::fs::read(file_path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes)?;

    Ok(text)
}
