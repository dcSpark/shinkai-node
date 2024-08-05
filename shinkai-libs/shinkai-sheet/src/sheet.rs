use async_channel::Sender;
use async_recursion::async_recursion;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::schemas::sheet::{
    Cell, CellId, CellStatus, ColumnBehavior, ColumnDefinition, ColumnIndex, ColumnUuid, RowIndex, RowUuid, UuidString,
    WorkflowSheetJobData,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{cell_name_converter::CellNameConverter, column_dependency_manager::ColumnDependencyManager};

const MAX_DEPENDENCY_DEPTH: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetUpdate {
    CellUpdated(RowUuid, ColumnUuid),
    // Add other update types as needed
}

pub trait SheetObserver {
    fn on_sheet_update(&self, update: SheetUpdate);
}

/// The `Sheet` struct represents the state of a spreadsheet.
/// This implementation uses a Redux-like architecture for managing state updates.
/// State updates are performed through actions, and the reducer function processes these actions to produce a new state.
/// Each action can also produce a list of jobs (side effects) that need to be executed asynchronously.
/// The state and jobs are returned as a tuple from the reducer, ensuring that the state is updated correctly
/// and the jobs can be processed externally.
#[derive(Serialize, Deserialize)]
pub struct Sheet {
    pub uuid: String,
    pub sheet_name: Option<String>,
    pub columns: HashMap<UuidString, ColumnDefinition>,
    pub rows: HashMap<UuidString, HashMap<UuidString, Cell>>,
    pub column_dependency_manager: ColumnDependencyManager,
    pub display_columns: Vec<UuidString>,
    pub display_rows: Vec<UuidString>,
    #[serde(skip_serializing, skip_deserializing)]
    update_sender: Option<Sender<SheetUpdate>>,
}

impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet")
            .field("uuid", &self.uuid)
            .field("sheet_name", &self.sheet_name)
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("column_dependency_manager", &self.column_dependency_manager)
            .field("display_columns", &self.display_columns)
            .field("display_rows", &self.display_rows)
            .field("observer", &"Option<Arc<Mutex<dyn SheetObserver>>>")
            .finish()
    }
}

impl Clone for Sheet {
    fn clone(&self) -> Self {
        Self {
            uuid: self.uuid.clone(),
            sheet_name: self.sheet_name.clone(),
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            column_dependency_manager: self.column_dependency_manager.clone(),
            display_columns: self.display_columns.clone(),
            display_rows: self.display_rows.clone(),
            update_sender: None, // Always set to None when cloning
        }
    }
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
            sheet_name: None,
            columns: HashMap::new(),
            rows: HashMap::new(),
            column_dependency_manager: ColumnDependencyManager::default(),
            display_columns: Vec::new(),
            display_rows: Vec::new(),
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
        let column_uuid = definition.id.clone();
        let mut jobs = self.dispatch(SheetAction::SetColumn(definition.clone())).await;

        if let ColumnBehavior::Formula(formula) = &definition.behavior {
            let dependencies = self.parse_formula_dependencies(formula);
            for dep in dependencies {
                self.column_dependency_manager.add_dependency(column_uuid.clone(), dep);
            }
        }

        self.display_columns.push(column_uuid.clone());

        // Collect rows to avoid borrowing issues
        let rows_to_update: Vec<UuidString> = self.display_rows.clone();

        for row in rows_to_update {
            if let ColumnBehavior::Formula(formula) = &definition.behavior {
                if let Some(value) = self.evaluate_formula(formula, row.clone(), column_uuid.clone()) {
                    let new_jobs = self
                        .dispatch(SheetAction::SetCellValue {
                            row: row.clone(),
                            col: column_uuid.clone(),
                            value,
                            input_hash: None,
                        })
                        .await;
                    jobs.extend(new_jobs);
                }
            }
        }

        // Trigger update for LLMCall columns only once
        if let ColumnBehavior::LLMCall { .. } = definition.behavior {
            let new_jobs = self
                .dispatch(SheetAction::TriggerUpdateColumnValues(column_uuid.clone()))
                .await;
            jobs.extend(new_jobs);
        }

        Ok(jobs)
    }

    pub async fn remove_column(&mut self, col_id: UuidString) -> Result<Vec<WorkflowSheetJobData>, String> {
        let jobs = self.dispatch(SheetAction::RemoveColumn(col_id)).await;
        Ok(jobs)
    }

    pub async fn add_row(&mut self, row_id: UuidString) -> Result<Vec<WorkflowSheetJobData>, String> {
        let jobs = self.dispatch(SheetAction::AddRow(row_id)).await;
        Ok(jobs)
    }

    pub fn parse_formula_dependencies(&self, formula: &str) -> HashSet<UuidString> {
        let mut dependencies = HashSet::new();

        // Check if the formula starts with '='
        if formula.starts_with('=') {
            let parts: Vec<&str> = formula[1..].split('+').collect();

            for part in parts {
                let part = part.trim();
                if !part.starts_with('"') || !part.ends_with('"') {
                    let col_index = CellNameConverter::column_name_to_index(part);
                    if let Some(col_uuid) = self.display_columns.get(col_index) {
                        dependencies.insert(col_uuid.clone());
                    }
                }
            }
        }

        dependencies
    }

    pub async fn remove_row(&mut self, row_id: UuidString) -> Result<Vec<WorkflowSheetJobData>, String> {
        let jobs = self.dispatch(SheetAction::RemoveRow(row_id)).await;
        Ok(jobs)
    }

    pub async fn set_cell_value(
        &mut self,
        row: UuidString,
        col: UuidString,
        value: String,
    ) -> Result<Vec<WorkflowSheetJobData>, String> {
        if !self.columns.contains_key(&col) {
            return Err("Column index out of bounds".to_string());
        }

        if !self.rows.contains_key(&row) {
            return Err("Row does not exist".to_string());
        }

        // these jobs is expected to always be empty. we are inserting a value into a cell
        let mut jobs = self
            .dispatch(SheetAction::SetCellValue {
                row: row.clone(),
                col: col.clone(),
                value,
                input_hash: None,
            })
            .await;

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

    pub fn evaluate_formula(&mut self, formula: &str, row: UuidString, col: UuidString) -> Option<String> {
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
                println!(
                    "Column name: {}, Column index: {}, Column id: {}",
                    col_name, col_index, col
                );
                if let Some(col_uuid) = self.display_columns.get(col_index) {
                    if let Some(value) = self.get_cell_value(row.clone(), col_uuid.clone()) {
                        result.push_str(&value);
                        dependencies.insert(col_uuid.clone());
                    }
                }
            } else if part.starts_with('"') && part.ends_with('"') {
                // Handle quoted strings
                let literal = &part[1..part.len() - 1];
                result.push_str(literal);
            } else {
                let col_index = CellNameConverter::column_name_to_index(part);
                println!("Column name: {}, Column: {}", part, col_index);
                if let Some(col_uuid) = self.display_columns.get(col_index) {
                    if let Some(value) = self.get_cell_value(row.clone(), col_uuid.clone()) {
                        result.push_str(&value);
                        dependencies.insert(col_uuid.clone());
                    }
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
    pub fn get_cell_value(&self, row: UuidString, col: UuidString) -> Option<String> {
        self.rows
            .get(&row)
            .and_then(|row_cells| row_cells.get(&col))
            .and_then(|cell| cell.value.clone())
    }

    pub fn cell_id_to_indices(&self, cell_id: &CellId) -> (UuidString, UuidString) {
        let parts: Vec<&str> = cell_id.0.split(':').collect();
        (parts[0].parse().unwrap(), parts[1].parse().unwrap())
    }

    // Additional helper methods
    pub fn get_cell(&self, row: UuidString, col: UuidString) -> Option<&Cell> {
        self.rows.get(&row).and_then(|row_cells| row_cells.get(&col))
    }

    pub fn get_column_definitions(&self) -> Vec<(UuidString, &ColumnDefinition)> {
        self.columns.iter().map(|(index, def)| (index.clone(), def)).collect()
    }

    fn get_column_formula(&self, col: UuidString) -> Option<String> {
        self.columns.get(&col).and_then(|col_def| {
            if let ColumnBehavior::Formula(formula) = &col_def.behavior {
                Some(formula.clone())
            } else {
                None
            }
        })
    }

    /// This function retrieves the input cells for a given column in a specific row.
    /// Inputs for a column are other cells that the column depends on, based on its behavior.
    /// For example, if the column has a formula, the input cells are those referenced in the formula.
    /// If the column is an LLMCall, the input cells are those referenced in the input string.
    pub fn get_input_cells_for_column(
        &self,
        row: UuidString,
        col: UuidString,
    ) -> Vec<(UuidString, UuidString, ColumnDefinition)> {
        let mut input_cells = Vec::new();

        if let Some(column_definition) = self.columns.get(&col) {
            match &column_definition.behavior {
                ColumnBehavior::Formula(formula) => {
                    let dependencies = self.parse_formula_dependencies(formula);
                    for dep_col in dependencies {
                        if let Some(dep_col_def) = self.columns.get(&dep_col) {
                            input_cells.push((row.clone(), dep_col.clone(), dep_col_def.clone()));
                        }
                    }
                }
                ColumnBehavior::LLMCall { input, .. } => {
                    let dependencies = self.parse_formula_dependencies(input);
                    for dep_col in dependencies {
                        if let Some(dep_col_def) = self.columns.get(&dep_col) {
                            input_cells.push((row.clone(), dep_col.clone(), dep_col_def.clone()));
                        }
                    }
                }
                _ => {}
            }
        }

        input_cells
    }

    /// Retrieves the input values for a given cell.
    ///
    /// # Arguments
    /// * `row` - Row index of the target cell
    /// * `col` - Column index of the target cell
    ///
    /// # Returns
    /// Vec of (ColumnIndex, Option<String>) pairs representing input cells and their values.
    pub fn get_input_values_for_cell(&self, row: UuidString, col: UuidString) -> Vec<(UuidString, Option<String>)> {
        let input_cells = self.get_input_cells_for_column(row.clone(), col.clone());
        input_cells
            .into_iter()
            .map(|(_, input_col, _)| (input_col.clone(), self.get_cell_value(row.clone(), input_col)))
            .collect()
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
        let mut headers: Vec<String> = self
            .display_columns
            .iter()
            .filter_map(|col_uuid| self.columns.get(col_uuid).map(|col_def| col_def.name.clone()))
            .collect();
        headers.insert(0, "Row".to_string());

        // Calculate the maximum width for each column
        let mut col_widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
        let max_row_len = self
            .display_rows
            .iter()
            .map(|row_uuid| row_uuid.len())
            .max()
            .unwrap_or(0);
        col_widths[0] = col_widths[0].max(max_row_len);

        for row_uuid in &self.display_rows {
            if let Some(row_cells) = self.rows.get(row_uuid) {
                for (col_index, col_uuid) in self.display_columns.iter().enumerate() {
                    let cell_value_len = row_cells
                        .get(col_uuid)
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
        for (index, row_uuid) in self.display_rows.iter().enumerate() {
            let short_row_uuid = format!("{} ({:.8}...)", index + 1, row_uuid);
            let mut row_data: Vec<String> = vec![format!("{:width$}", short_row_uuid, width = col_widths[0])];
            if let Some(row_cells) = self.rows.get(row_uuid) {
                for (col_index, col_uuid) in self.display_columns.iter().enumerate() {
                    let cell_value = row_cells
                        .get(col_uuid)
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
        row: UuidString,
        col: UuidString,
        value: String,
        input_hash: Option<String>,
    },
    TriggerUpdateEvent {
        changed_cell_id: CellId,
        visited: HashSet<(UuidString, UuidString)>,
        depth: usize,
    },
    RemoveColumn(UuidString),
    TriggerUpdateColumnValues(UuidString),
    RemoveRow(UuidString),
    AddRow(UuidString), // Add other actions as needed
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
                    state
                        .column_dependency_manager
                        .add_dependency(definition.id.clone(), dep);
                }
            }
            state.columns.insert(definition.id.clone(), definition);
        }
        SheetAction::SetCellValue {
            row,
            col,
            value,
            input_hash,
        } => {
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
                    input_hash,
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
            eprintln!("TriggerUpdateEvent: {:?}", changed_cell_id);

            if !visited.insert((row.clone(), col.clone())) {
                eprintln!("Circular dependency detected at cell ({}, {})", row, col);
                return (state, jobs);
            }

            let dependents = state.column_dependency_manager.get_reverse_dependents(col.clone());
            eprintln!("Col: {:?} Dependents: {:?}", col, dependents);
            for dependent_col in dependents {
                if let Some(column_definition) = state.columns.get(&dependent_col).cloned() {
                    match &column_definition.behavior {
                        ColumnBehavior::Formula(formula) => {
                            if let Some(value) = state.evaluate_formula(formula, row.clone(), dependent_col.clone()) {
                                let (new_state, mut new_jobs) = sheet_reducer(
                                    state,
                                    SheetAction::SetCellValue {
                                        row: row.clone(),
                                        col: dependent_col.clone(),
                                        value,
                                        input_hash: None,
                                    },
                                )
                                .await;
                                state = new_state;
                                jobs.append(&mut new_jobs);

                                eprintln!("row: {:?}, dep_col: {:?}, col: {:?}", row, dependent_col, col);
                                let new_cell_id = CellId(format!("{}:{}", row, dependent_col));
                                eprintln!("TriggerUpdateEvent newcellid: {:?}", new_cell_id);
                                if changed_cell_id != new_cell_id {
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
                        }
                        ColumnBehavior::LLMCall {
                            input: _, // used under the hood with get_input_cells_for_column
                            workflow,
                            llm_provider_name,
                            input_hash,
                        } => {
                            // Check if input_hash is present and matches the blake3 hash of the current input cells values
                            let input_cells = state.get_input_cells_for_column(row.clone(), dependent_col.clone());
                            let workflow_job_data = WorkflowSheetJobData {
                                sheet_id: state.uuid.clone(),
                                row: row.clone(),
                                col: dependent_col.clone(),
                                col_definition: column_definition.clone(),
                                workflow: workflow.clone(),
                                input_cells,
                                llm_provider_name: llm_provider_name.clone(),
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
        SheetAction::RemoveColumn(col_uuid) => {
            // Get dependents before removing the column
            let dependents = state.column_dependency_manager.get_reverse_dependents(col_uuid.clone());

            // Remove the column
            state.columns.remove(&col_uuid);
            for row in state.rows.values_mut() {
                row.remove(&col_uuid);
            }
            state.column_dependency_manager.remove_column(col_uuid.clone());

            // Remove the column from display_columns
            state.display_columns.retain(|uuid| uuid != &col_uuid);

            // Trigger updates for columns dependent on the removed column
            for dependent_col in dependents {
                for row_uuid in state.rows.keys().cloned().collect::<Vec<_>>() {
                    if let Some(column_definition) = state.columns.get(&dependent_col).cloned() {
                        if let ColumnBehavior::Formula(formula) = &column_definition.behavior {
                            if let Some(value) =
                                state.evaluate_formula(formula, row_uuid.clone(), dependent_col.clone())
                            {
                                let (new_state, mut new_jobs) = sheet_reducer(
                                    state,
                                    SheetAction::SetCellValue {
                                        row: row_uuid.clone(),
                                        col: dependent_col.clone(),
                                        value,
                                        input_hash: None,
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
        }
        SheetAction::TriggerUpdateColumnValues(col_uuid) => {
            for row_uuid in state.rows.keys() {
                if let Some(column_definition) = state.columns.get(&col_uuid).cloned() {
                    if let ColumnBehavior::LLMCall {
                        input: _, // used under the hood with get_input_cells_for_column
                        workflow,
                        llm_provider_name,
                        input_hash: _,
                    } = &column_definition.behavior
                    {
                        let input_cells = state.get_input_cells_for_column(row_uuid.clone(), col_uuid.clone());
                        let workflow_job_data = WorkflowSheetJobData {
                            sheet_id: state.uuid.clone(),
                            row: row_uuid.clone(),
                            col: col_uuid.clone(),
                            col_definition: column_definition.clone(),
                            workflow: workflow.clone(),
                            input_cells,
                            llm_provider_name: llm_provider_name.clone(),
                        };

                        jobs.push(workflow_job_data);
                    }
                }
            }
        }
        SheetAction::RemoveRow(row_uuid) => {
            state.rows.remove(&row_uuid);
            state.display_rows.retain(|uuid| uuid != &row_uuid);
            // Optionally, you can add logic to handle dependencies or other side effects
        }
        SheetAction::AddRow(row_uuid) => {
            eprintln!("SheetAction::AddRow: {:?}", row_uuid);
            if state.rows.contains_key(&row_uuid) {
                return (state, jobs); // Row already exists, return current state
            }

            let mut row_cells = HashMap::new();
            for (col_uuid, col_def) in &state.columns {
                if let ColumnBehavior::Text = col_def.behavior {
                    row_cells.insert(
                        col_uuid.clone(),
                        Cell {
                            value: Some("".to_string()), // Default empty value for text columns
                            last_updated: Utc::now(),
                            status: CellStatus::Ready,
                            input_hash: None,
                        },
                    );
                }
            }
            state.rows.insert(row_uuid.clone(), row_cells);
            state.display_rows.push(row_uuid.clone());

            // Collect update events for non-text columns
            let mut update_events = Vec::new();
            for (col_uuid, col_def) in &state.columns {
                if let ColumnBehavior::Text = col_def.behavior {
                    continue; // Skip text columns
                }

                let changed_cell_id = CellId(format!("{}:{}", row_uuid, col_uuid));
                eprintln!("\nAddRow TriggerUpdateEvent: {:?}", changed_cell_id);
                update_events.push(SheetAction::TriggerUpdateEvent {
                    changed_cell_id,
                    visited: HashSet::new(),
                    depth: 0,
                });
            }

            // Apply update events
            for event in update_events {
                let (new_state, mut new_jobs) = sheet_reducer(state.clone(), event).await;
                eprintln!("update_events New state: {:?}", new_state);
                state = new_state;
                jobs.append(&mut new_jobs);
            }
        }
    }
    (state, jobs)
}
