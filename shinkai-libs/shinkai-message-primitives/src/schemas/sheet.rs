use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;

pub type RowIndex = usize;
pub type ColumnIndex = usize;
pub type RowUuid = String;
pub type ColumnUuid = String;
pub type Formula = String;
pub type FilePath = String;
pub type FileName = String;
pub type UuidString = String;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct APIColumnDefinition {
    pub id: Option<UuidString>,
    pub name: Option<String>,
    pub behavior: ColumnBehavior,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ColumnDefinition {
    pub id: UuidString,
    pub name: String,
    pub behavior: ColumnBehavior,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ColumnBehavior {
    Text,
    Number,
    Formula(String),
    LLMCall {
        input: Formula,
        workflow: Workflow,
        llm_provider_name: String, // Note: maybe we want a duality: specific model or some rules that pick a model e.g. Cheap + Private
        input_hash: Option<String>, // New parameter to store the hash of inputs (avoid recomputation)
    },
    MultipleVRFiles {
        files: Vec<(FilePath, FileName)>,
    },
    // TODO: Add support for uploaded files. Specify String
    UploadedFiles {
        files: Vec<String>, // Mocking uploaded files as strings
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum CellStatus {
    Pending,
    Ready,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Cell {
    pub value: Option<String>,
    pub last_updated: DateTime<Utc>,
    pub status: CellStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct CellId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowSheetJobData {
    pub sheet_id: UuidString,
    pub row: UuidString,
    pub col: UuidString,
    pub col_definition: ColumnDefinition,
    pub workflow: Workflow,
    pub llm_provider_name: String,
    pub input_cells: Vec<(UuidString, UuidString, ColumnDefinition)>,
}