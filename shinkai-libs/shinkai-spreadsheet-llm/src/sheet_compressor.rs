use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io::Cursor,
};

use csv::ReaderBuilder;
use ndarray::Array1;

const K: usize = 4;

pub struct SheetCompressor {
    row_candidates: Vec<usize>,
    column_candidates: Vec<usize>,
    row_lengths: HashMap<usize, usize>,
    col_lengths: HashMap<usize, usize>,
}

pub struct MarkdownCell {
    address: String,
    value: String,
    format: String,
}

impl SheetCompressor {
    pub fn new() -> Self {
        Self {
            row_candidates: Vec::new(),
            column_candidates: Vec::new(),
            row_lengths: HashMap::new(),
            col_lengths: HashMap::new(),
        }
    }

    // Vanilla Spreadsheet Encoding to Markdown
    pub fn encode(csv_data: &Vec<u8>) -> Result<HashMap<(usize, usize), MarkdownCell>, Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut cells = HashMap::new();

        let records = reader.records().collect::<Result<Vec<_>, _>>()?;
        let num_rows = records.len();
        for (i, record) in records.into_iter().enumerate() {
            let row: Vec<String> = record.iter().map(String::from).collect();
            for (j, value) in row.iter().enumerate() {
                let address = Self::convert_rowcol_to_excel_index(i, j);

                let format = {
                    let mut format = Vec::new();
                    if i == 0 {
                        format.push("Left Border".to_string());
                    }
                    if j == 0 {
                        format.push("Top Border".to_string());
                    }
                    if i == num_rows - 1 {
                        format.push("Bottom Border".to_string());
                    }
                    if j == row.len() - 1 {
                        format.push("Right Border".to_string());
                    }
                    format.join(", ")
                };

                let cell = MarkdownCell {
                    address,
                    value: value.clone(),
                    format,
                };

                cells.insert((i, j), cell);
            }
        }

        Ok(cells)
    }

    // Structural-anchor-based Extraction
    pub fn anchor(&mut self, csv_data: &Vec<u8>) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let records = reader.records().collect::<Result<Vec<_>, _>>()?;
        let csv_rows = records
            .iter()
            .map(|r| r.iter().map(String::from).collect())
            .collect::<Vec<Vec<String>>>();
        let num_rows = csv_rows.len();
        let num_cols = csv_rows.get(0).unwrap_or(&vec![]).len();

        self.get_dtype_row(csv_data)?;
        self.get_dtype_column(csv_data)?;
        self.get_length_row(csv_data)?;
        self.get_length_col(csv_data)?;

        // Keep candidates found in both dtype/length method
        self.row_candidates = Self::intersect1d(
            &Array1::from(self.row_lengths.keys().cloned().collect::<Vec<usize>>()),
            &Array1::from(self.row_candidates.clone()),
        );
        self.column_candidates = Self::intersect1d(
            &Array1::from(self.col_lengths.keys().cloned().collect::<Vec<usize>>()),
            &Array1::from(self.column_candidates.clone()),
        );

        // Add first and last row/column as candidates
        self.row_candidates = ndarray::stack![
            ndarray::Axis(0),
            self.row_candidates,
            Array1::from(vec![0, num_rows - 1])
        ]
        .into_iter()
        .collect();
        self.column_candidates = ndarray::stack![
            ndarray::Axis(0),
            self.column_candidates,
            Array1::from(vec![0, num_cols - 1])
        ]
        .into_iter()
        .collect();

        // Get K closest rows/columns to each candidate
        let mut concatenated: Vec<usize> = vec![];
        for i in &self.row_candidates {
            concatenated.extend(Self::surrounding_k(*i, K));
        }

        let array = Array1::from(concatenated);
        let unique: HashSet<_> = array.iter().cloned().collect();
        self.row_candidates = unique.into_iter().collect();

        concatenated = vec![];
        for i in &self.column_candidates {
            concatenated.extend(Self::surrounding_k(*i, K));
        }

        let array = Array1::from(concatenated);
        let unique: HashSet<_> = array.iter().cloned().collect();
        self.column_candidates = unique.into_iter().collect();

        // Filter out invalid candidates
        self.row_candidates = self
            .row_candidates
            .clone()
            .into_iter()
            .filter(|&x| x < num_rows)
            .collect();
        self.column_candidates = self
            .column_candidates
            .clone()
            .into_iter()
            .filter(|&x| x < num_cols)
            .collect();

        let result: Vec<Vec<String>> = self
            .row_candidates
            .iter()
            .map(|&row| {
                self.column_candidates
                    .iter()
                    .map(|&col| csv_rows[row][col].clone())
                    .collect()
            })
            .collect();

        Ok(result)
    }

    fn convert_rowcol_to_excel_index(row: usize, col: usize) -> String {
        let mut result = String::new();
        let mut col = col + 1;
        let row = row + 1;

        while col > 0 {
            let rem = (col - 1) % 26;
            result.push((rem as u8 + b'A') as char);
            col = (col - rem) / 26;
        }

        format!("{}{}", result.chars().rev().collect::<String>(), &row.to_string())
    }

    fn get_dtype_row(&mut self, csv_data: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut current_type: Vec<String> = Vec::new();
        for (i, record) in reader.records().enumerate() {
            let row: Vec<String> = record?.iter().map(String::from).collect();
            let temp: Vec<String> = row.iter().map(|s| Self::detect_type(s).to_string()).collect();
            if current_type != temp {
                current_type = temp;
                self.row_candidates.push(i);
            }
        }

        Ok(())
    }

    fn get_dtype_column(&mut self, csv_data: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut current_type: Vec<String> = Vec::new();
        let mut columns: Vec<Vec<String>> = Vec::new();
        for (i, record) in reader.records().enumerate() {
            let row: Vec<String> = record?.iter().map(String::from).collect();
            for (j, value) in row.iter().enumerate() {
                if i == 0 {
                    columns.push(Vec::new());
                }
                columns[j].push(value.clone());
            }
        }

        for (i, column) in columns.iter().enumerate() {
            let temp: Vec<String> = column.iter().map(|s| Self::detect_type(s).to_string()).collect();
            if current_type != temp {
                current_type = temp;
                self.column_candidates.push(i);
            }
        }

        Ok(())
    }

    fn get_length_row(&mut self, csv_data: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut row_lengths = HashMap::new();
        for (i, record) in reader.records().enumerate() {
            let row: Vec<String> = record?.iter().map(String::from).collect();
            let row_length = row
                .iter()
                .map(|s| match Self::detect_type(s) {
                    "string" => s.len(),
                    _ => 0,
                })
                .sum::<usize>();
            row_lengths.insert(i, row_length);
        }

        let lengths: Vec<usize> = row_lengths.values().cloned().collect();
        let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
        let std = (lengths.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / lengths.len() as f64).sqrt();
        let min = (mean - 2.0 * std).max(0.0);
        let max = mean + 2.0 * std;

        self.row_lengths = row_lengths
            .into_iter()
            .filter(|&(_, v)| ((v as f64) < min) || ((v as f64) > max))
            .collect();

        Ok(())
    }

    fn get_length_col(&mut self, csv_data: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut col_lengths = HashMap::new();
        let mut columns: Vec<Vec<String>> = Vec::new();
        for (i, record) in reader.records().enumerate() {
            let row: Vec<String> = record?.iter().map(String::from).collect();
            for (j, value) in row.iter().enumerate() {
                if i == 0 {
                    columns.push(Vec::new());
                }
                columns[j].push(value.clone());
            }
        }

        for (i, column) in columns.iter().enumerate() {
            let col_length = column
                .iter()
                .map(|s| match Self::detect_type(s) {
                    "string" => s.len(),
                    _ => 0,
                })
                .sum::<usize>();
            col_lengths.insert(i, col_length);
        }

        let lengths: Vec<usize> = col_lengths.values().cloned().collect();
        let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
        let std = (lengths.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / lengths.len() as f64).sqrt();
        let min = (mean - 2.0 * std).max(0.0);
        let max = mean + 2.0 * std;

        self.col_lengths = col_lengths
            .into_iter()
            .filter(|&(_, v)| ((v as f64) < min) || ((v as f64) > max))
            .collect();

        Ok(())
    }

    fn detect_type(input: &str) -> &str {
        if input.parse::<i64>().is_ok() {
            "integer"
        } else if input.parse::<f64>().is_ok() {
            "float"
        } else if input.parse::<bool>().is_ok() {
            "bool"
        } else {
            "string"
        }
    }

    fn intersect1d(arr1: &Array1<usize>, arr2: &Array1<usize>) -> Vec<usize> {
        let set1: HashSet<_> = arr1.iter().cloned().collect();
        let set2: HashSet<_> = arr2.iter().cloned().collect();
        set1.intersection(&set2).cloned().collect()
    }

    fn surrounding_k(num: usize, k: usize) -> Vec<usize> {
        (num - k..=num + k).collect()
    }
}
