use std::collections::HashMap;

use polars::frame::DataFrame;

#[derive(Debug, Clone)]
pub enum ResponseType {
    String(String),
    KeyPoints(Vec<KeyPoint>),
}

#[derive(Debug, Clone)]
pub enum ContextData {
    String(String),
    DataFrames(Vec<DataFrame>),
    Dictionary(HashMap<String, DataFrame>),
}

#[derive(Debug, Clone)]
pub enum ContextText {
    String(String),
    Strings(Vec<String>),
    Dictionary(HashMap<String, String>),
}

#[derive(Debug, Clone)]
pub struct KeyPoint {
    pub answer: String,
    pub score: i32,
}
