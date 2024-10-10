use chrono::NaiveDateTime;
use ndarray::Array1;
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self},
};

use crate::excel_helpers::{combine_cells, convert_rowcol_to_excel_index};

// Structure-anchor Threshold
const K: usize = 4;

pub struct SheetCompressor {}

#[derive(Debug, Clone)]
pub struct MarkdownCell {
    pub address: String,
    pub value: String,
    pub category: String,
}

pub struct CompressedSheet {
    pub areas: CellAreas,
    pub dictionary: IndexDictionary,
    pub markdown: Vec<MarkdownCell>,
}

pub type SheetRows = Vec<Vec<String>>;
pub type CellAreas = Vec<((usize, usize), (usize, usize), String)>;

pub struct IndexDictionary(HashMap<String, String>);

impl fmt::Display for IndexDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();
        for (key, value) in &self.0 {
            result.push_str(&format!("({}|{})\n", key, value));
        }
        write!(f, "{}", result)
    }
}

impl FromIterator<(String, String)> for IndexDictionary {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut map = HashMap::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        IndexDictionary(map)
    }
}

impl SheetCompressor {
    pub fn compress_sheet(sheet: &SheetRows) -> CompressedSheet {
        // Structural-anchor-based Extraction
        // let sheet = Self::anchor(sheet);

        // Encode sheet to Markdown
        let markdown = Self::encode(&sheet);

        // Inverted-index Translation
        let dictionary = Self::inverted_index(markdown.clone());

        // Data-format-aware Aggregation
        let areas = Self::identical_cell_aggregation(sheet, markdown.clone());

        CompressedSheet {
            areas,
            dictionary,
            markdown,
        }
    }

    // Vanilla Spreadsheet Encoding to Markdown
    pub fn encode(sheet: &SheetRows) -> Vec<MarkdownCell> {
        let mut cells = Vec::new();

        for (i, row) in sheet.iter().enumerate() {
            for (j, value) in row.iter().enumerate() {
                let address = convert_rowcol_to_excel_index(i, j);

                let cell = MarkdownCell {
                    address,
                    value: value.clone(),

                    category: Self::get_category(value).to_string(),
                };

                cells.push(cell);
            }
        }

        cells
    }

    // Structural-anchor-based Extraction
    pub fn anchor(sheet: &SheetRows) -> SheetRows {
        let num_rows = sheet.len();
        let num_cols = sheet.get(0).unwrap_or(&vec![]).len();

        let mut row_candidates = Self::get_dtype_row(sheet);
        let mut column_candidates = Self::get_dtype_column(sheet);
        let row_lengths = Self::get_length_row(sheet);
        let col_lengths = Self::get_length_col(sheet);

        // Keep candidates found in both dtype/length method
        row_candidates = Self::intersect1d(
            &Array1::from(row_lengths.keys().cloned().collect::<Vec<usize>>()),
            &Array1::from(row_candidates.clone()),
        );
        column_candidates = Self::intersect1d(
            &Array1::from(col_lengths.keys().cloned().collect::<Vec<usize>>()),
            &Array1::from(column_candidates.clone()),
        );

        // Add first and last row/column as candidates
        row_candidates.extend_from_slice(&[0, num_rows - 1]);
        column_candidates.extend_from_slice(&[0, num_cols - 1]);

        // Get K closest rows/columns to each candidate
        let mut concatenated: Vec<usize> = vec![];
        for i in &row_candidates {
            concatenated.extend(Self::surrounding_k(*i, K));
        }

        let array = Array1::from(concatenated);
        let unique: HashSet<_> = array.iter().cloned().collect();
        row_candidates = unique.into_iter().collect();

        concatenated = vec![];
        for i in &column_candidates {
            concatenated.extend(Self::surrounding_k(*i, K));
        }

        let array = Array1::from(concatenated);
        let unique: HashSet<_> = array.iter().cloned().collect();
        column_candidates = unique.into_iter().collect();

        // Filter out invalid candidates
        row_candidates = row_candidates.clone().into_iter().filter(|&x| x < num_rows).collect();
        column_candidates = column_candidates
            .clone()
            .into_iter()
            .filter(|&x| x < num_cols)
            .collect();

        let result: SheetRows = row_candidates
            .iter()
            .map(|&row| column_candidates.iter().map(|&col| sheet[row][col].clone()).collect())
            .collect();

        result
    }

    // Inverted-index Translation
    pub fn inverted_index(markdown: Vec<MarkdownCell>) -> IndexDictionary {
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
        sheet: &SheetRows,
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

    fn get_dtype_row(sheet: &SheetRows) -> Vec<usize> {
        let mut row_candidates: Vec<usize> = Vec::new();
        let mut current_type: Vec<String> = Vec::new();
        for (i, row) in sheet.iter().enumerate() {
            let temp: Vec<String> = row.iter().map(|s| Self::detect_type(s).to_string()).collect();
            if current_type != temp {
                current_type = temp;
                row_candidates.push(i);
            }
        }

        row_candidates
    }

    fn get_dtype_column(sheet: &SheetRows) -> Vec<usize> {
        let mut column_candidates: Vec<usize> = Vec::new();
        let mut current_type: Vec<String> = Vec::new();
        let mut columns: Vec<Vec<String>> = Vec::new();
        for (i, row) in sheet.iter().enumerate() {
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
                column_candidates.push(i);
            }
        }

        column_candidates
    }

    fn get_length_row(sheet: &SheetRows) -> HashMap<usize, usize> {
        let mut row_lengths = HashMap::new();
        for (i, row) in sheet.iter().enumerate() {
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

        row_lengths = row_lengths
            .into_iter()
            .filter(|&(_, v)| ((v as f64) < min) || ((v as f64) > max))
            .collect();

        row_lengths
    }

    fn get_length_col(sheet: &SheetRows) -> HashMap<usize, usize> {
        let mut col_lengths = HashMap::new();
        let mut columns: Vec<Vec<String>> = Vec::new();
        for (i, row) in sheet.iter().enumerate() {
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

        col_lengths = col_lengths
            .into_iter()
            .filter(|&(_, v)| ((v as f64) < min) || ((v as f64) > max))
            .collect();

        col_lengths
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
        if let Ok(_) = string.parse::<i64>() {
            return "Integer";
        }
        if let Ok(_) = string.parse::<f64>() {
            return "Float";
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

        "String"
    }

    fn intersect1d(arr1: &Array1<usize>, arr2: &Array1<usize>) -> Vec<usize> {
        let set1: HashSet<_> = arr1.iter().cloned().collect();
        let set2: HashSet<_> = arr2.iter().cloned().collect();
        set1.intersection(&set2).cloned().collect()
    }

    fn surrounding_k(num: usize, k: usize) -> Vec<usize> {
        let start = (num as isize - k as isize).max(0) as usize;
        (start..=num + k).collect()
    }

    fn dfs(
        sheet: &SheetRows,
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
