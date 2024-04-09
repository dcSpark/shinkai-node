// /// Parse CSV data from a buffer and attempt to automatically detect
// /// headers.
// pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VRError> {
//     let mut reader = Reader::from_reader(Cursor::new(buffer));
//     let headers = reader
//         .headers()
//         .map_err(|_| VRError::FailedCSVParsing)?
//         .iter()
//         .map(String::from)
//         .collect::<Vec<String>>();

//     let likely_header = headers.iter().all(|s| {
//         let is_alphabetic = s.chars().all(|c| c.is_alphabetic() || c.is_whitespace());
//         let no_duplicates = headers.iter().filter(|&item| item == s).count() == 1;
//         let no_prohibited_chars = !s.contains(&['@', '#', '$', '%', '^', '&', '*'][..]);

//         is_alphabetic && no_duplicates && no_prohibited_chars
//     });

//     Self::parse_csv(&buffer, likely_header)
// }

// /// Parse CSV data from a buffer.
// /// * `header` - A boolean indicating whether to prepend column headers to
// ///   values.
// pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VRError> {
//     let mut reader = Reader::from_reader(Cursor::new(buffer));
//     let headers = if header {
//         reader
//             .headers()
//             .map_err(|_| VRError::FailedCSVParsing)?
//             .iter()
//             .map(String::from)
//             .collect::<Vec<String>>()
//     } else {
//         Vec::new()
//     };

//     let mut result = Vec::new();
//     for record in reader.records() {
//         let record = record.map_err(|_| VRError::FailedCSVParsing)?;
//         let row: Vec<String> = if header {
//             record
//                 .iter()
//                 .enumerate()
//                 .map(|(i, e)| format!("{}: {}", headers[i], e))
//                 .collect()
//         } else {
//             record.iter().map(String::from).collect()
//         };
//         let row_string = row.join(", ");
//         result.push(row_string);
//     }

//     Ok(result)
// }
