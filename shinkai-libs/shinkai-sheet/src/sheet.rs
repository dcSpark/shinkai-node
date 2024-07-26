use async_channel::Sender;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    cell_name_converter::CellNameConverter, column_dependency_manager::ColumnDependencyManager, sheet_job::SheetJob,
};

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

    pub fn set_column(&mut self, definition: ColumnDefinition) {
        if let ColumnBehavior::Formula(ref formula) = definition.behavior {
            let dependencies = self.parse_formula_dependencies(formula);
            for dep in dependencies {
                self.column_dependency_manager.add_dependency(definition.id, dep);
            }
        }
        self.columns.insert(definition.id, definition);
    }

    pub fn parse_formula_dependencies(&self, formula: &str) -> HashSet<ColumnIndex> {
        let parts: Vec<&str> = formula.split('+').collect();
        let mut dependencies = HashSet::new();

        for part in parts {
            let part = part.trim();
            if part.starts_with('=') {
                let cell_name = &part[1..];
                let (_, col_index) = CellNameConverter::cell_name_to_indices(cell_name);
                dependencies.insert(col_index);
            } else if !part.starts_with('"') || !part.ends_with('"') {
                let (_, col_index) = CellNameConverter::cell_name_to_indices(part);
                dependencies.insert(col_index);
            }
        }

        dependencies
    }

    pub async fn set_cell_value(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        value: String,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Result<(), String> {
        if !self.columns.contains_key(&col) {
            return Err("Column index out of bounds".to_string());
        }

        let row_cells = self.rows.entry(row).or_default();
        row_cells.insert(
            col,
            Cell {
                value: Some(value),
                last_updated: Utc::now(),
                status: CellStatus::Ready,
            },
        );

        let changed_cell_id = CellId(format!("{}:{}", row, col));
        self.trigger_update_event(&changed_cell_id, workflow_job_creator).await;
        Ok(())
    }

    pub async fn trigger_update_event(
        &mut self,
        changed_cell_id: &CellId,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) {
        let (row, col) = self.cell_id_to_indices(changed_cell_id);
        let dependents = self.column_dependency_manager.get_dependents(col);
        for dependent_col in dependents {
            self.update_cell(row, dependent_col, workflow_job_creator); // Note: need to fix this one
        }
        if let Some(sender) = &self.update_sender {
            sender.send(SheetUpdate::CellUpdated(row, col)).await.unwrap();
        }
    }

    pub async fn update_cell(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow_job_creator: &dyn WorkflowJobCreator,
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

                let job =
                    self.initiate_workflow_job(row, col, &workflow, &[], &llm_provider_name, workflow_job_creator);
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
                let cell_name = &part[1..];
                let (row_index, col_index) = CellNameConverter::cell_name_to_indices(cell_name);
                println!("Cell name: {}, Row: {}, Column: {}", cell_name, row_index, col_index);
                if let Some(value) = self.get_cell_value(row_index, col_index) {
                    result.push_str(&value);
                    dependencies.insert(col_index);
                }
            } else if part.starts_with('"') && part.ends_with('"') {
                // Handle quoted strings
                let literal = &part[1..part.len() - 1];
                result.push_str(literal);
            } else {
                let (row_index, col_index) = CellNameConverter::cell_name_to_indices(part);
                println!("Cell name: {}, Row: {}, Column: {}", part, row_index, col_index);
                if let Some(value) = self.get_cell_value(row_index, col_index) {
                    result.push_str(&value);
                    dependencies.insert(col_index);
                }
            }
        }

        // Update column dependencies
        for dep in dependencies {
            self.column_dependency_manager.add_dependency(col, dep);
        }

        println!("Formula result: {}", result);
        Some(result)
    }

    pub fn initiate_workflow_job(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow: &Workflow,
        input_columns: &[ColumnIndex],
        llm_provider_name: &str,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Box<dyn SheetJob> {
        let input_values: Vec<String> = input_columns
            .iter()
            .filter_map(|&col_id| self.get_cell_value(row, col_id))
            .collect();

        workflow_job_creator.initiate_workflow_job(row, col, workflow, input_columns, &input_values)
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
}

#[cfg(test)]
mod tests {
    use shinkai_dsl::parser::parse_workflow;

    use crate::sheet_job::MockupSheetJob;

    use super::*;

    struct MockWorkflowJobCreator {
        sheet: Arc<Mutex<Sheet>>,
    }

    impl MockWorkflowJobCreator {
        fn new(sheet: Arc<Mutex<Sheet>>) -> Self {
            Self { sheet }
        }

        async fn complete_job(&self, cell_id: CellId, result: String) {
            let (row, col) = {
                let sheet = self.sheet.lock().await;
                sheet.cell_id_to_indices(&cell_id)
            };

            let mut sheet = self.sheet.lock().await;
            sheet
                .set_cell_value(row, col, result, self)
                .await
                .expect("Failed to set cell value");
        }
    }

    impl WorkflowJobCreator for MockWorkflowJobCreator {
        fn initiate_workflow_job(
            &self,
            row: usize,
            col: usize,
            _workflow: &Workflow,
            input_columns: &[usize],
            _cell_values: &[String],
        ) -> Box<dyn SheetJob> {
            let cell_id = CellId(format!("{}:{}", row, col));
            let cell_id_clone = cell_id.clone();
            let result = "Job Result".to_string();
            let sheet = self.sheet.clone();
            tokio::spawn(async move {
                // Simulate job completion
                let creator = MockWorkflowJobCreator { sheet };
                creator.complete_job(cell_id, result).await;
            });

            Box::new(MockupSheetJob::new(
                "mock_job_id".to_string(),
                cell_id_clone,
                "".to_string(),
                input_columns.iter().map(|&col| CellId(col.to_string())).collect(),
            ))
        }
    }

    #[tokio::test]
    async fn test_llm_call_column() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let column_text = ColumnDefinition {
            id: 0,
            name: "Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let workflow_str = r#"
            workflow WorkflowTest v0.1 {
                step Main {
                    $RESULT = call opinionated_inference($INPUT)
                }
            }
        "#;
        let workflow = parse_workflow(workflow_str).unwrap();

        let column_llm = ColumnDefinition {
            id: 1,
            name: "LLM Call Column".to_string(),
            behavior: ColumnBehavior::LLMCall {
                input: "Say Hello World".to_string(),
                workflow,
                llm_provider_name: "MockProvider".to_string(),
            },
        };

        {
            let mut sheet = sheet.lock().await;
            sheet.set_column(column_text.clone());
            sheet.set_column(column_llm.clone());
        }

        assert_eq!(sheet.lock().await.columns.len(), 2);
        assert_eq!(sheet.lock().await.columns[&0], column_text);
        assert_eq!(sheet.lock().await.columns[&1], column_llm);

        let workflow_job_creator = MockWorkflowJobCreator::new(sheet.clone());
        let jobs = sheet.lock().await.update_cell(0, 1, &workflow_job_creator).await;

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id(), "mock_job_id");

        // Check the value of the cell after the update
        let cell_value = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value, None);

        {
            let sheet_locked = sheet.lock().await;
            let cell_status = sheet_locked.get_cell(0, 1).map(|cell| &cell.status);
            assert_eq!(cell_status, Some(&CellStatus::Pending));
        }

        // Simulate job completion
        workflow_job_creator
            .complete_job(CellId("0:1".to_string()), "Hello World".to_string())
            .await;

        // Check the value of the cell after the job completion
        let cell_value = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value, Some("Hello World".to_string()));

        {
            let sheet_locked = sheet.lock().await;
            let cell_status = sheet_locked.get_cell(0, 1).map(|cell| &cell.status);
            assert_eq!(cell_status, Some(&CellStatus::Ready));
        }
    }
}
