use chrono::NaiveDateTime;
use csv::ReaderBuilder;
use ndarray::Array1;
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io::Cursor,
};

use crate::excel_helpers::{combine_cells, convert_rowcol_to_excel_index};

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
    category: String,
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
    pub fn encode(csv_data: &Vec<u8>) -> Result<Vec<MarkdownCell>, Box<dyn Error>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_reader(Cursor::new(csv_data));

        let mut cells = Vec::new();

        let records = reader.records().collect::<Result<Vec<_>, _>>()?;
        let num_rows = records.len();
        for (i, record) in records.into_iter().enumerate() {
            let row: Vec<String> = record.iter().map(String::from).collect();
            for (j, value) in row.iter().enumerate() {
                let address = convert_rowcol_to_excel_index(i, j);

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
                    category: Self::get_category(value).to_string(),
                };

                cells.push(cell);
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

    // Inverted-index Translation
    pub fn inverted_index(markdown: Vec<MarkdownCell>) -> HashMap<String, String> {
        let mut dictionary: HashMap<String, Vec<String>> = HashMap::new();

        for row in markdown.iter() {
            let value = row.value.clone();
            let address = row.address.clone();

            dictionary
                .entry(value)
                .and_modify(|e| e.push(address.clone()))
                .or_insert_with(|| vec![address]);
        }

        dictionary.retain(|k, _| !k.is_empty());

        dictionary.into_iter().map(|(k, v)| (k, combine_cells(v))).collect()
    }

    // Data-format-aware Aggregation
    pub fn identical_cell_aggregation(
        sheet: Vec<Vec<String>>,
        markdown: Vec<MarkdownCell>,
    ) -> Vec<((usize, usize), (usize, usize), String)> {
        let dictionary = Self::inverted_category(markdown);
        let other = "Other".to_string();

        let m = sheet.len();
        let n = sheet[0].len();

        let mut visited = vec![vec![false; n]; m];
        let mut areas = Vec::new();

        for r in 0..m {
            for c in 0..n {
                if !visited[r][c] {
                    let val_type = dictionary.get(&sheet[r][c]).unwrap_or(&other);
                    let bounds = Self::dfs(&sheet, &dictionary, r, c, val_type, &mut visited);
                    areas.push(((bounds.0, bounds.1), (bounds.2, bounds.3), val_type.to_string()));
                }
            }
        }

        areas
    }

    fn inverted_category(markdown: Vec<MarkdownCell>) -> HashMap<String, String> {
        let mut dictionary: HashMap<String, String> = HashMap::new();

        for row in markdown.iter() {
            let category = row.category.clone();
            let value = row.value.clone();

            dictionary.insert(value, category);
        }

        dictionary
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

    fn get_category(string: &str) -> &str {
        if string.is_empty() {
            return "Other";
        }
        if let Ok(_) = string.parse::<f64>() {
            return "Float";
        }
        if let Ok(_) = string.parse::<i64>() {
            return "Integer";
        }
        if let Ok(_) = NaiveDateTime::parse_from_str(string, "%Y-%m-%d %H:%M:%S") {
            return "yyyy/mm/dd";
        }

        let integer_re = Regex::new(r"^(\+|-)?\d+$").unwrap();
        let formatted_integer_re = Regex::new(r"^\d{1,3}(,\d{1,3})*$").unwrap();
        if integer_re.is_match(string) || formatted_integer_re.is_match(string) {
            return "Integer";
        }

        let float_re = Regex::new(r"^[-+]?\d*\.?\d*$").unwrap();
        let formatted_float_re = Regex::new(r"^\d{1,3}(,\d{3})*(\.\d+)?$").unwrap();
        if float_re.is_match(string) || formatted_float_re.is_match(string) {
            return "Float";
        }

        let percentage_re = Regex::new(r"^[-+]?\d*\.?\d*%$").unwrap();
        let formatted_percentage_re = Regex::new(r"^\d{1,3}(,\d{3})*(\.\d+)?%$").unwrap();
        if percentage_re.is_match(string) || formatted_percentage_re.is_match(string) {
            return "Percentage";
        }

        let currency_re = Regex::new(r"^[-+]?[$]\d*\.?\d{2}$").unwrap();
        let formatted_currency_re = Regex::new(r"^[-+]?[$]\d{1,3}(,\d{3})*(\.\d{2})?$").unwrap();
        if currency_re.is_match(string) || formatted_currency_re.is_match(string) {
            return "Currency";
        }

        let scientific_notation_re = Regex::new(r"\b-?1-9?[Ee][-+]?\d+\b").unwrap();
        if scientific_notation_re.is_match(string) {
            return "Scientific Notation";
        }

        let email_re = Regex::new(r"^((([!#$%&'*+\-/=?^_`{|}~\w])|([!#$%&'*+\-/=?^_`{|}~\w][!#$%&'*+\-/=?^_`{|}~\.\w]{0,}[!#$%&'*+\-/=?^_`{|}~\w]))[@]\w+([-.]\w+)*\.\w+([-.]\w+)*)$").unwrap();
        if email_re.is_match(string) {
            return "Email";
        }

        if let Ok(_) = NaiveDateTime::parse_from_str(string, "%Y-%m-%d %H:%M:%S") {
            return "yyyy/mm/dd";
        }

        "Other"
    }

    fn intersect1d(arr1: &Array1<usize>, arr2: &Array1<usize>) -> Vec<usize> {
        let set1: HashSet<_> = arr1.iter().cloned().collect();
        let set2: HashSet<_> = arr2.iter().cloned().collect();
        set1.intersection(&set2).cloned().collect()
    }

    fn surrounding_k(num: usize, k: usize) -> Vec<usize> {
        (num - k..=num + k).collect()
    }

    fn dfs(
        sheet: &Vec<Vec<String>>,
        dictionary: &HashMap<String, String>,
        r: usize,
        c: usize,
        val_type: &str,
        visited: &mut Vec<Vec<bool>>,
    ) -> (usize, usize, usize, usize) {
        let other = "Other".to_string();
        let match_val = dictionary.get(&sheet[r][c]).unwrap_or(&other);
        if visited[r][c] || val_type != match_val {
            return (r, c, r.wrapping_sub(1), c.wrapping_sub(1));
        }
        visited[r][c] = true;
        let mut bounds = (r, c, r, c);
        let directions = [(-1, 0), (0, -1), (1, 0), (0, 1)];

        for &(dr, dc) in &directions {
            let new_r = (r as isize + dr) as usize;
            let new_c = (c as isize + dc) as usize;
            if new_r < sheet.len() && new_c < sheet[0].len() {
                let match_val = dictionary.get(&sheet[new_r][new_c]).unwrap_or(&other);
                if !visited[new_r][new_c] && val_type == match_val {
                    let new_bounds = Self::dfs(sheet, dictionary, new_r, new_c, val_type, visited);
                    bounds = (
                        bounds.0.min(new_bounds.0),
                        bounds.1.min(new_bounds.1),
                        bounds.2.max(new_bounds.2),
                        bounds.3.max(new_bounds.3),
                    );
                }
            }
        }
        bounds
    }
}
