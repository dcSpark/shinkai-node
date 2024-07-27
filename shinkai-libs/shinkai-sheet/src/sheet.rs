use async_channel::Sender;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;
use std::any::Any;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    cell_name_converter::CellNameConverter, column_dependency_manager::ColumnDependencyManager, sheet_job::SheetJob,
};

const MAX_DEPENDENCY_DEPTH: usize = 20;

pub type RowIndex = usize;
pub type ColumnIndex = usize;
pub type Formula = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetUpdate {
    CellUpdated(RowIndex, ColumnIndex),
    // Add other update types as needed
}

pub trait SheetObserver {
    fn on_sheet_update(&self, update: SheetUpdate);
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ColumnDefinition {
    pub id: usize,
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
        llm_provider_name: String,
    },
    SingleFile {
        path: String,
        name: String,
    },
    MultipleFiles {
        files: Vec<(String, String)>, // (path, name)
    },
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

#[derive(Serialize, Deserialize)]
pub struct Sheet {
    pub uuid: String,
    pub columns: HashMap<usize, ColumnDefinition>,
    pub rows: HashMap<RowIndex, HashMap<ColumnIndex, Cell>>,
    pub column_dependency_manager: ColumnDependencyManager,
    #[serde(skip_serializing, skip_deserializing)]
    update_sender: Option<Sender<SheetUpdate>>,
}

impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet")
            .field("uuid", &self.uuid)
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("column_dependency_manager", &self.column_dependency_manager)
            .field("observer", &"Option<Arc<Mutex<dyn SheetObserver>>>")
            .finish()
    }
}

impl Clone for Sheet {
    fn clone(&self) -> Self {
        Self {
            uuid: self.uuid.clone(),
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            column_dependency_manager: self.column_dependency_manager.clone(),
            update_sender: None, // Always set to None when cloning
        }
    }
}

pub trait WorkflowJobCreator: Send + Sync {
    fn initiate_workflow_job(
        &self,
        row: usize,
        col: usize,
        workflow: &Workflow,
        input_columns: &[usize],
        cell_values: &[String],
    ) -> Box<dyn SheetJob>;

    // Used for testing
    fn as_any(&self) -> &dyn Any;
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Sheet {
    pub fn new() -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            columns: HashMap::new(),
            rows: HashMap::new(),
            column_dependency_manager: ColumnDependencyManager::default(),
            update_sender: None,
        }
    }

    pub fn set_update_sender(&mut self, sender: Sender<SheetUpdate>) {
        self.update_sender = Some(sender);
    }

    pub fn dispatch(&mut self, action: SheetAction) {
        *self = sheet_reducer(self.clone(), action);
    }

    pub fn set_column(&mut self, definition: ColumnDefinition) {
        self.dispatch(SheetAction::SetColumn(definition.clone()));

        if let ColumnBehavior::Formula(formula) = &definition.behavior {
            let dependencies = self.parse_formula_dependencies(formula);
            for dep in dependencies {
                self.column_dependency_manager.add_dependency(definition.id, dep);
            }
        }

        // TODO: Add a on / off for pause. so we can add new columns without auto-populating

        // Auto-populate cells for the new column based on existing active rows
        let active_rows: Vec<RowIndex> = if self.rows.is_empty() {
            vec![0] // Ensure at least the first row is active
        } else {
            self.rows.keys().cloned().collect()
        };

        for row in active_rows {
            if let ColumnBehavior::Formula(formula) = &definition.behavior {
                if let Some(value) = self.evaluate_formula(formula, row, definition.id) {
                    self.dispatch(SheetAction::SetCellValue {
                        row,
                        col: definition.id,
                        value,
                    });
                }
            }
        }
    }

    pub fn parse_formula_dependencies(&self, formula: &str) -> HashSet<ColumnIndex> {
        let parts: Vec<&str> = formula.split('+').collect();
        let mut dependencies = HashSet::new();

        for part in parts {
            let part = part.trim();
            if part.starts_with('=') {
                let col_name = &part[1..];
                dependencies.insert(CellNameConverter::column_name_to_index(col_name));
            } else if !part.starts_with('"') || !part.ends_with('"') {
                dependencies.insert(CellNameConverter::column_name_to_index(part));
            }
        }

        dependencies
    }

    pub async fn set_cell_value(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        value: String,
        workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
    ) -> Result<(), String> {
        if !self.columns.contains_key(&col) {
            return Err("Column index out of bounds".to_string());
        }

        self.dispatch(SheetAction::SetCellValue { row, col, value });

        let changed_cell_id = CellId(format!("{}:{}", row, col));
        self.dispatch(SheetAction::TriggerUpdateEvent {
            changed_cell_id,
            workflow_job_creator,
            visited: HashSet::new(),
            depth: 0,
        });
        Ok(())
    }

    pub async fn trigger_update_event(
        &mut self,
        changed_cell_id: &CellId,
        workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
    ) {
        let (row, col) = self.cell_id_to_indices(changed_cell_id);
        let dependents = self.column_dependency_manager.get_dependents(col);
        for dependent_col in dependents {
            self.update_cell(row, dependent_col, workflow_job_creator.clone()).await;
        }
        if let Some(sender) = &self.update_sender {
            sender.send(SheetUpdate::CellUpdated(row, col)).await.unwrap();
        }
    }

    pub async fn update_cell(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
    ) -> Vec<Box<dyn SheetJob>> {
        eprintln!("Updating cell: {} {}", row, col);
        let mut jobs = Vec::new();
        let column_behavior = self.columns.get(&col).unwrap().behavior.clone();
        match column_behavior {
            ColumnBehavior::Formula(formula) => {
                if let Some(value) = self.evaluate_formula(&formula, row, col) {
                    self.set_cell_value(row, col, value, workflow_job_creator)
                        .await
                        .unwrap();
                }
            }
            ColumnBehavior::LLMCall {
                input,
                workflow,
                llm_provider_name,
            } => {
                // Set cell status to Pending
                eprintln!("Setting cell status to Pending");
                let row_cells = self.rows.entry(row).or_default();
                if let Some(cell) = row_cells.get_mut(&col) {
                    eprintln!("Setting cell status to Pending");
                    cell.status = CellStatus::Pending;
                } else {
                    eprintln!("Creating new cell with status Pending");
                    // If the cell does not exist, create it with Pending status
                    row_cells.insert(
                        col,
                        Cell {
                            value: None,
                            last_updated: Utc::now(),
                            status: CellStatus::Pending,
                        },
                    );
                }

                let job = self
                    .initiate_workflow_job(row, col, &workflow, &[], &llm_provider_name, workflow_job_creator)
                    .await;
                jobs.push(job);
            }
            ColumnBehavior::SingleFile { path, name } => {
                println!("Single file: {} ({})", name, path);
            }
            ColumnBehavior::MultipleFiles { files } => {
                for (path, name) in files {
                    println!("File: {} ({})", name, path);
                }
            }
            ColumnBehavior::UploadedFiles { files } => {
                for file in files {
                    println!("Uploaded file: {}", file);
                }
            }
            _ => {}
        }
        jobs
    }

    pub fn evaluate_formula(&mut self, formula: &str, row: RowIndex, col: ColumnIndex) -> Option<String> {
        println!("Evaluating formula: {}", formula);
        let parts: Vec<&str> = formula.split('+').collect();
        let mut result = String::new();
        let mut dependencies = HashSet::new();

        for part in parts {
            let part = part.trim();
            println!("Processing part: {}", part);
            if part.starts_with('=') {
                let col_name = &part[1..];
                let col_index = CellNameConverter::column_name_to_index(col_name);
                println!("Column name: {}, Column: {}", col_name, col_index);
                if let Some(value) = self.get_cell_value(row, col_index) {
                    result.push_str(&value);
                    dependencies.insert(col_index);
                }
            } else if part.starts_with('"') && part.ends_with('"') {
                // Handle quoted strings
                let literal = &part[1..part.len() - 1];
                result.push_str(literal);
            } else {
                let col_index = CellNameConverter::column_name_to_index(part);
                println!("Column name: {}, Column: {}", part, col_index);
                if let Some(value) = self.get_cell_value(row, col_index) {
                    result.push_str(&value);
                    dependencies.insert(col_index);
                }
            }
        }

        self.column_dependency_manager.update_dependencies(col, dependencies);
        Some(result)
    }

    pub async fn initiate_workflow_job(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow: &Workflow,
        input_columns: &[ColumnIndex],
        llm_provider_name: &str,
        workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
    ) -> Box<dyn SheetJob> {
        let input_values: Vec<String> = input_columns
            .iter()
            .filter_map(|&col_id| self.get_cell_value(row, col_id))
            .collect();

        let creator = workflow_job_creator.lock().await;
        creator.initiate_workflow_job(row, col, workflow, input_columns, &input_values)
    }

    pub fn get_cell_value(&self, row: RowIndex, col: ColumnIndex) -> Option<String> {
        self.rows
            .get(&row)
            .and_then(|row_cells| row_cells.get(&col))
            .and_then(|cell| cell.value.clone())
    }

    pub fn cell_id_to_indices(&self, cell_id: &CellId) -> (usize, usize) {
        let parts: Vec<&str> = cell_id.0.split(':').collect();
        (parts[0].parse().unwrap(), parts[1].parse().unwrap())
    }

    // Additional helper methods
    pub fn get_cell(&self, row: RowIndex, col: ColumnIndex) -> Option<&Cell> {
        self.rows.get(&row).and_then(|row_cells| row_cells.get(&col))
    }

    pub fn get_column_definitions(&self) -> Vec<(ColumnIndex, &ColumnDefinition)> {
        self.columns.iter().map(|(&index, def)| (index, def)).collect()
    }

    fn get_column_formula(&self, col: ColumnIndex) -> Option<String> {
        self.columns.get(&col).and_then(|col_def| {
            if let ColumnBehavior::Formula(formula) = &col_def.behavior {
                Some(formula.clone())
            } else {
                None
            }
        })
    }

    pub fn print_as_ascii_table(&self) {
        // Collect column headers in order
        let mut headers: Vec<String> = (0..self.columns.len())
            .filter_map(|i| self.columns.get(&i).map(|col_def| col_def.name.clone()))
            .collect();
        headers.insert(0, "Row".to_string());
    
        // Calculate the maximum width for each column
        let mut col_widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
        let max_row = *self.rows.keys().max().unwrap_or(&0);
        col_widths[0] = col_widths[0].max(max_row.to_string().len());
    
        for row_index in 0..=max_row {
            if let Some(row_cells) = self.rows.get(&row_index) {
                for col_index in 0..self.columns.len() {
                    let cell_value_len = row_cells.get(&col_index)
                        .and_then(|cell| cell.value.as_ref().map(|v| v.len()))
                        .unwrap_or(0);
                    col_widths[col_index + 1] = col_widths[col_index + 1].max(cell_value_len);
                }
            }
        }
    
        // Print headers with padding
        let header_line: Vec<String> = headers
            .iter()
            .enumerate()
            .map(|(i, header)| format!("{:width$}", header, width = col_widths[i]))
            .collect();
        println!("{}", header_line.join(" | "));
        println!("{}", "-".repeat(header_line.join(" | ").len()));
    
        // Print rows with padding
        for row_index in 0..=max_row {
            let mut row_data: Vec<String> = vec![format!("{:width$}", row_index, width = col_widths[0])];
            if let Some(row_cells) = self.rows.get(&row_index) {
                for col_index in 0..self.columns.len() {
                    let cell_value = row_cells
                        .get(&col_index)
                        .and_then(|cell| cell.value.clone())
                        .unwrap_or_else(|| "".to_string());
                    row_data.push(format!("{:width$}", cell_value, width = col_widths[col_index + 1]));
                }
                println!("{}", row_data.join(" | "));
            }
        }
        println!();
        println!();
    }
}

#[derive(Clone)]
pub enum SheetAction {
    SetColumn(ColumnDefinition),
    SetCellValue {
        row: RowIndex,
        col: ColumnIndex,
        value: String,
    },
    TriggerUpdateEvent {
        changed_cell_id: CellId,
        workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
        visited: HashSet<(RowIndex, ColumnIndex)>,
        depth: usize,
    },
    // Add other actions as needed
}

impl std::fmt::Debug for SheetAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SheetAction::SetColumn(definition) => f.debug_struct("SetColumn").field("definition", definition).finish(),
            SheetAction::SetCellValue { row, col, value } => f
                .debug_struct("SetCellValue")
                .field("row", row)
                .field("col", col)
                .field("value", value)
                .finish(),
            SheetAction::TriggerUpdateEvent { changed_cell_id, .. } => f
                .debug_struct("TriggerUpdateEvent")
                .field("changed_cell_id", changed_cell_id)
                .field("workflow_job_creator", &"Arc<Mutex<Box<dyn WorkflowJobCreator>>>")
                .finish(),
            // Add other actions as needed
        }
    }
}

// Implement the reducer function
pub fn sheet_reducer(mut state: Sheet, action: SheetAction) -> Sheet {
    //  if std::env::var("LOG_REDUX").is_ok() {
    println!("<Sheet Reducer>");
    println!("Dispatching action: {:?}", action);
    println!("Current state: \n");
    state.print_as_ascii_table();
    // }

    match action {
        SheetAction::SetColumn(definition) => {
            if let ColumnBehavior::Formula(ref formula) = definition.behavior {
                let dependencies = state.parse_formula_dependencies(formula);
                for dep in dependencies {
                    state.column_dependency_manager.add_dependency(definition.id, dep);
                }
            }
            state.columns.insert(definition.id, definition);
        }
        SheetAction::SetCellValue { row, col, value } => {
            if !state.columns.contains_key(&col) {
                return state; // Column index out of bounds
            }

            let row_cells = state.rows.entry(row).or_default();
            row_cells.insert(
                col,
                Cell {
                    value: Some(value),
                    last_updated: Utc::now(),
                    status: CellStatus::Ready,
                },
            );
        }
        SheetAction::TriggerUpdateEvent {
            changed_cell_id,
            workflow_job_creator,
            mut visited,
            depth,
        } => {
            if depth >= MAX_DEPENDENCY_DEPTH {
                eprintln!("Maximum dependency depth reached. Possible circular dependency detected.");
                return state;
            }

            let (row, col) = state.cell_id_to_indices(&changed_cell_id);

            if !visited.insert((row, col)) {
                eprintln!("Circular dependency detected at cell ({}, {})", row, col);
                return state;
            }

            let dependents = state.column_dependency_manager.get_reverse_dependents(col);
            eprintln!("Col: {:?} Dependents: {:?}", col, dependents);
            for dependent_col in dependents {
                if let Some(formula) = state.get_column_formula(dependent_col) {
                    if let Some(value) = state.evaluate_formula(&formula, row, dependent_col) {
                        state = sheet_reducer(
                            state,
                            SheetAction::SetCellValue {
                                row,
                                col: dependent_col,
                                value,
                            },
                        );
                        let new_cell_id = CellId(format!("{}:{}", row, dependent_col));
                        state = sheet_reducer(
                            state,
                            SheetAction::TriggerUpdateEvent {
                                changed_cell_id: new_cell_id,
                                workflow_job_creator: workflow_job_creator.clone(),
                                visited: visited.clone(),
                                depth: depth + 1,
                            },
                        );
                    }
                }
            }

            if let Some(sender) = &state.update_sender {
                let sender_clone = sender.clone();
                tokio::spawn(async move {
                    sender_clone.send(SheetUpdate::CellUpdated(row, col)).await.unwrap();
                });
            }
        } // Handle other actions
    }
    state
}
