use async_channel::Sender;
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{cell_name_converter::CellNameConverter, column_dependency_manager::ColumnDependencyManager};

const MAX_DEPENDENCY_DEPTH: usize = 20;

pub type RowIndex = usize;
pub type ColumnIndex = usize;
pub type Formula = String;
pub type FilePath = String;
pub type FileName = String;

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
        input: Formula, // Note: Maybe actually not needed?
        workflow: Workflow,
        llm_provider_name: String, // Note: maybe we want a duality: specific model or some rules that pick a model e.g. Cheap + Private
        input_hash: Option<String>, // New parameter to store the hash of inputs (avoid recomputation)
    },
    // TODO: merge single and multiple files into a single enum
    SingleFile {
        path: String,
        name: String,
    },
    MultipleFiles {
        files: Vec<(String, String)>, // (path, name)
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

/// The `Sheet` struct represents the state of a spreadsheet.
/// This implementation uses a Redux-like architecture for managing state updates.
/// State updates are performed through actions, and the reducer function processes these actions to produce a new state.
/// Each action can also produce a list of jobs (side effects) that need to be executed asynchronously.
/// The state and jobs are returned as a tuple from the reducer, ensuring that the state is updated correctly
/// and the jobs can be processed externally.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowSheetJobData {
    pub row: RowIndex,
    pub col: ColumnIndex,
    pub col_definition: ColumnDefinition,
    pub workflow: Workflow,
    pub input_cells: Vec<(RowIndex, ColumnIndex, ColumnDefinition)>,
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

    pub async fn dispatch(&mut self, action: SheetAction) -> Vec<WorkflowSheetJobData> {
        let (new_state, jobs) = sheet_reducer(self.clone(), action).await;
        *self = new_state;
        jobs
    }

    pub async fn set_column(&mut self, definition: ColumnDefinition) -> Result<Vec<WorkflowSheetJobData>, String> {
        let mut jobs = self.dispatch(SheetAction::SetColumn(definition.clone())).await;

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
                    let new_jobs = self
                        .dispatch(SheetAction::SetCellValue {
                            row,
                            col: definition.id,
                            value,
                        })
                        .await;
                    jobs.extend(new_jobs);
                }
            }
        }

        Ok(jobs)
    }

    pub async fn remove_column(&mut self, col_index: ColumnIndex) -> Result<(), String> {
        self.dispatch(SheetAction::RemoveColumn(col_index)).await;
        Ok(())
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
    ) -> Result<Vec<WorkflowSheetJobData>, String> {
        if !self.columns.contains_key(&col) {
            return Err("Column index out of bounds".to_string());
        }

        // this jobs is expected to always be empty. we are inserting a value into a cell
        let mut jobs = self.dispatch(SheetAction::SetCellValue { row, col, value }).await;

        let changed_cell_id = CellId(format!("{}:{}", row, col));
        // unlike the previous job, this one *may* have updates because some formulas or workflows may depend on us
        // so updating x,y cell may trigger workflow(s) that depends on x,y
        let new_jobs = self
            .dispatch(SheetAction::TriggerUpdateEvent {
                changed_cell_id,
                visited: HashSet::new(),
                depth: 0,
            })
            .await;

        jobs.extend(new_jobs);
        Ok(jobs)
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

    // Note: if we create a workflow in B that depends on A (not defined)
    // then we should skip the workflow creation and set the cell to Pending
    // and then when A is defined, we should trigger the workflow
    // Same goes for A -> B -> C

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

    #[allow(dead_code)]
    fn get_column_formula(&self, col: ColumnIndex) -> Option<String> {
        self.columns.get(&col).and_then(|col_def| {
            if let ColumnBehavior::Formula(formula) = &col_def.behavior {
                Some(formula.clone())
            } else {
                None
            }
        })
    }

    fn get_input_cells_for_column(
        &self,
        row: RowIndex,
        col: ColumnIndex,
    ) -> Vec<(RowIndex, ColumnIndex, ColumnDefinition)> {
        let mut input_cells = Vec::new();

        if let Some(column_definition) = self.columns.get(&col) {
            match &column_definition.behavior {
                ColumnBehavior::Formula(formula) => {
                    let dependencies = self.parse_formula_dependencies(formula);
                    for dep_col in dependencies {
                        if let Some(dep_col_def) = self.columns.get(&dep_col) {
                            input_cells.push((row, dep_col, dep_col_def.clone()));
                        }
                    }
                }
                ColumnBehavior::LLMCall { input, .. } => {
                    let dependencies = self.parse_formula_dependencies(input);
                    for dep_col in dependencies {
                        if let Some(dep_col_def) = self.columns.get(&dep_col) {
                            input_cells.push((row, dep_col, dep_col_def.clone()));
                        }
                    }
                }
                _ => {}
            }
        }

        input_cells
    }

    fn compute_input_hash(
        &self,
        input_cells: &[(RowIndex, ColumnIndex, ColumnDefinition)],
        workflow: &Workflow,
    ) -> Option<String> {
        if input_cells.is_empty() {
            return None;
        }

        let mut inputs: Vec<String> = input_cells
            .iter()
            .map(|(row, col, _)| format!("{}:{}", row, col))
            .collect();
        inputs.sort();
        let concatenated = inputs.join(",");
        let workflow_key = workflow.generate_key();
        Some(
            blake3::hash(format!("{}::{}", concatenated, workflow_key).as_bytes())
                .to_hex()
                .to_string(),
        )
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
                    let cell_value_len = row_cells
                        .get(&col_index)
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

// Note: add a method that can compact the sheet state in a way that a job can check if it's still valid or it can be completed
// with the current state

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SheetAction {
    SetColumn(ColumnDefinition),
    SetCellValue {
        row: RowIndex,
        col: ColumnIndex,
        value: String,
    },
    TriggerUpdateEvent {
        changed_cell_id: CellId,
        visited: HashSet<(RowIndex, ColumnIndex)>,
        depth: usize,
    },
    RemoveColumn(ColumnIndex),
    // Add other actions as needed
}

// Implement the reducer function
#[async_recursion]
pub async fn sheet_reducer(mut state: Sheet, action: SheetAction) -> (Sheet, Vec<WorkflowSheetJobData>) {
    //  if std::env::var("LOG_REDUX").is_ok() {
    println!("<Sheet Reducer>");
    println!("Dispatching action: {:?}", action);
    println!("Current state: \n");
    state.print_as_ascii_table();
    // }

    let mut jobs = Vec::new();
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
                return (state, jobs); // Column index out of bounds
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
            mut visited,
            depth,
        } => {
            if depth >= MAX_DEPENDENCY_DEPTH {
                eprintln!("Maximum dependency depth reached. Possible circular dependency detected.");
                return (state, jobs);
            }

            let (row, col) = state.cell_id_to_indices(&changed_cell_id);

            if !visited.insert((row, col)) {
                eprintln!("Circular dependency detected at cell ({}, {})", row, col);
                return (state, jobs);
            }

            let dependents = state.column_dependency_manager.get_reverse_dependents(col);
            eprintln!("Col: {:?} Dependents: {:?}", col, dependents);
            for dependent_col in dependents {
                if let Some(column_definition) = state.columns.get(&dependent_col).cloned() {
                    match &column_definition.behavior {
                        ColumnBehavior::Formula(formula) => {
                            if let Some(value) = state.evaluate_formula(formula, row, dependent_col) {
                                let (new_state, mut new_jobs) = sheet_reducer(
                                    state,
                                    SheetAction::SetCellValue {
                                        row,
                                        col: dependent_col,
                                        value,
                                    },
                                )
                                .await;
                                state = new_state;
                                jobs.append(&mut new_jobs);

                                let new_cell_id = CellId(format!("{}:{}", row, dependent_col));
                                let (new_state, mut new_jobs) = sheet_reducer(
                                    state,
                                    SheetAction::TriggerUpdateEvent {
                                        changed_cell_id: new_cell_id,
                                        visited: visited.clone(),
                                        depth: depth + 1,
                                    },
                                )
                                .await;
                                state = new_state;
                                jobs.append(&mut new_jobs);
                            }
                        }
                        ColumnBehavior::LLMCall {
                            input,
                            workflow,
                            llm_provider_name,
                            input_hash,
                        } => {
                            let input_cells = state.get_input_cells_for_column(row, dependent_col);
                            let workflow_job_data = WorkflowSheetJobData {
                                row,
                                col: dependent_col,
                                col_definition: column_definition.clone(),
                                workflow: workflow.clone(),
                                input_cells,
                            };

                            jobs.push(workflow_job_data);
                        }
                        _ => {}
                    }
                }
            }

            if let Some(sender) = &state.update_sender {
                let sender_clone = sender.clone();
                tokio::spawn(async move {
                    sender_clone.send(SheetUpdate::CellUpdated(row, col)).await.unwrap();
                });
            }
        }
        SheetAction::RemoveColumn(col_index) => {
            // Get dependents before removing the column
            let dependents = state.column_dependency_manager.get_reverse_dependents(col_index);

            // Remove the column
            state.columns.remove(&col_index);
            for row in state.rows.values_mut() {
                row.remove(&col_index);
            }
            state.column_dependency_manager.remove_column(col_index);

            // Re-order the columns
            let mut new_columns = HashMap::new();
            for (old_index, col_def) in state.columns.iter() {
                let new_index = if *old_index > col_index {
                    old_index - 1
                } else {
                    *old_index
                };
                new_columns.insert(
                    new_index,
                    ColumnDefinition {
                        id: new_index,
                        ..col_def.clone()
                    },
                );
            }
            state.columns = new_columns;

            // Re-order the rows
            for row in state.rows.values_mut() {
                let mut new_row = HashMap::new();
                for (old_index, cell) in row.iter() {
                    let new_index = if *old_index > col_index {
                        old_index - 1
                    } else {
                        *old_index
                    };
                    new_row.insert(new_index, cell.clone());
                }
                *row = new_row;
            }

            // Trigger updates for columns dependent on the removed column
            for dependent_col in dependents {
                for row_index in state.rows.keys().cloned().collect::<Vec<_>>() {
                    let new_col_index = if dependent_col > col_index {
                        dependent_col - 1
                    } else {
                        dependent_col
                    };
                    if let Some(column_definition) = state.columns.get(&new_col_index).cloned() {
                        if let ColumnBehavior::Formula(formula) = &column_definition.behavior {
                            if let Some(value) = state.evaluate_formula(formula, row_index, new_col_index) {
                                let (new_state, mut new_jobs) = sheet_reducer(
                                    state,
                                    SheetAction::SetCellValue {
                                        row: row_index,
                                        col: new_col_index,
                                        value,
                                    },
                                )
                                .await;
                                state = new_state;
                                jobs.append(&mut new_jobs);
                            }
                        }
                    }
                }
            }
        } // Handle other actions
    }
    (state, jobs)
}
