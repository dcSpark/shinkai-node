use std::{error::Error, io::Cursor};

use polars::{
    frame::DataFrame,
    io::SerReader,
    prelude::{CsvReader, NamedFrom},
    series::Series,
};

pub struct SheetCompressor {
    row_candidates: Vec<usize>,
    column_candidates: Vec<usize>,
}

pub struct MarkdownCell {
    address: (usize, usize),
    value: String,
    format: String,
}

impl SheetCompressor {
    pub fn new() -> Self {
        Self {
            row_candidates: Vec::new(),
            column_candidates: Vec::new(),
        }
    }

    pub fn encode(csv_data: Vec<u8>) -> Result<DataFrame, Box<dyn Error>> {
        let df = CsvReader::new(Cursor::new(csv_data)).finish()?;

        let mut cells = Vec::new();

        for (i, row) in df.iter().enumerate() {
            for (j, series) in row.iter().enumerate() {
                let value = series.to_string();
                cells.push(MarkdownCell {
                    address: (i, j),
                    value,
                    format: "".to_string(),
                });
            }
        }

        let s_address = Series::new(
            "Address",
            cells
                .iter()
                .map(|cell| Self::convert_rowcol_to_excel_index(cell.address.0, cell.address.1))
                .collect::<Vec<String>>(),
        );
        let s_value = Series::new(
            "Value",
            cells.iter().map(|cell| cell.value.clone()).collect::<Vec<String>>(),
        );
        let s_format = Series::new(
            "Format",
            cells.iter().map(|cell| cell.format.clone()).collect::<Vec<String>>(),
        );

        Ok(DataFrame::new(vec![s_address, s_value, s_format])?)
    }

    fn convert_rowcol_to_excel_index(row: usize, col: usize) -> String {
        let mut result = String::new();
        let mut col = col + 1;

        while col > 0 {
            let rem = (col - 1) % 26;
            result.push((rem as u8 + b'A') as char);
            col = (col - rem) / 26;
        }

        format!("{}{}", result.chars().rev().collect::<String>(), &row.to_string())
    }
}
