use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use shinkai_dsl::dsl_schemas::Workflow;
use std::collections::{HashMap, HashSet};

use crate::{cell_name_converter::CellNameConverter, column_dependency_manager::ColumnDependencyManager, sheet_job::SheetJob};

pub type RowIndex = usize;
pub type ColumnIndex = usize;
pub type Formula = String;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ColumnDefinition {
    id: usize,
    name: String,
    behavior: ColumnBehavior,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ColumnBehavior {
    Text,
    Number,
    Formula(String),
    LLMCall {
        input: Formula,
        workflow: Workflow,
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Cell {
    value: Option<String>,
    last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct CellId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sheet {
    columns: HashMap<usize, ColumnDefinition>,
    rows: HashMap<RowIndex, HashMap<ColumnIndex, Cell>>,
    column_dependency_manager: ColumnDependencyManager,
}

pub trait WorkflowJobCreator {
    fn create_workflow_job(
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
            columns: HashMap::new(),
            rows: HashMap::new(),
            column_dependency_manager: ColumnDependencyManager::default(),
        }
    }

    pub fn add_column(&mut self, definition: ColumnDefinition) {
        if let ColumnBehavior::Formula(ref formula) = definition.behavior {
            let dependencies = self.parse_formula_dependencies(formula);
            for dep in dependencies {
                self.column_dependency_manager.add_dependency(definition.id, dep);
            }
        }
        self.columns.insert(definition.id, definition);
    }

    fn parse_formula_dependencies(&self, formula: &str) -> HashSet<ColumnIndex> {
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

    fn set_cell_value(
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
            },
        );

        let changed_cell_id = CellId(format!("{}:{}", row, col));
        self.trigger_update_event(&changed_cell_id, workflow_job_creator);
        Ok(())
    }

    fn trigger_update_event(&mut self, changed_cell_id: &CellId, workflow_job_creator: &dyn WorkflowJobCreator) {
        let (row, col) = self.cell_id_to_indices(changed_cell_id);
        let dependents = self.column_dependency_manager.get_dependents(col);
        for dependent_col in dependents {
            self.update_cell(row, dependent_col, workflow_job_creator);
        }
    }

    fn update_cell(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Vec<Box<dyn SheetJob>> {
        let mut jobs = Vec::new();
        let column_behavior = self.columns.get(&col).unwrap().behavior.clone();
        match column_behavior {
            ColumnBehavior::Formula(formula) => {
                if let Some(value) = self.evaluate_formula(&formula, row, col) {
                    self.set_cell_value(row, col, value, workflow_job_creator).unwrap();
                }
            }
            ColumnBehavior::LLMCall { input, workflow } => {
                let job = self.create_workflow_job(row, col, &workflow, &[], workflow_job_creator);
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

    fn evaluate_formula(&mut self, formula: &str, row: RowIndex, col: ColumnIndex) -> Option<String> {
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

    fn create_workflow_job(
        &mut self,
        row: RowIndex,
        col: ColumnIndex,
        workflow: &Workflow,
        input_columns: &[ColumnIndex],
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Box<dyn SheetJob> {
        let input_values: Vec<String> = input_columns
            .iter()
            .filter_map(|&col_id| self.get_cell_value(row, col_id))
            .collect();

        // Update column dependencies
        for &input_col in input_columns {
            self.column_dependency_manager.add_dependency(col, input_col);
        }

        workflow_job_creator.create_workflow_job(row, col, workflow, input_columns, &input_values)
    }

    fn get_cell_value(&self, row: RowIndex, col: ColumnIndex) -> Option<String> {
        self.rows
            .get(&row)
            .and_then(|row_cells| row_cells.get(&col))
            .and_then(|cell| cell.value.clone())
    }

    fn cell_id_to_indices(&self, cell_id: &CellId) -> (usize, usize) {
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
    use crate::sheet_job::MockupSheetJob;

    use super::*;
    use chrono::Utc;

    struct MockWorkflowJobCreator;

    impl WorkflowJobCreator for MockWorkflowJobCreator {
        fn create_workflow_job(
            &self,
            row: usize,
            col: usize,
            _workflow: &Workflow,
            input_columns: &[usize],
            _cell_values: &[String],
        ) -> Box<dyn SheetJob> {
            Box::new(MockupSheetJob::new(
                "mock_job_id".to_string(),
                CellId(format!("{}:{}", row, col)),
                "".to_string(),
                input_columns.iter().map(|&col| CellId(col.to_string())).collect(),
            ))
        }
    }

    #[test]
    fn test_add_column() {
        let mut sheet = Sheet::new();
        let column = ColumnDefinition {
            id: 0,
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        sheet.add_column(column.clone());
        assert_eq!(sheet.columns.len(), 1);
        assert_eq!(sheet.columns[&0], column);
    }

    #[test]
    fn test_set_cell_value() {
        let mut sheet = Sheet::new();
        let column = ColumnDefinition {
            id: 0,
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        sheet.add_column(column);

        let workflow_job_creator = MockWorkflowJobCreator;
        let result = sheet.set_cell_value(0, 0, "Test Value".to_string(), &workflow_job_creator);
        assert!(result.is_ok());

        let cell = sheet.get_cell(0, 0).unwrap();
        assert_eq!(cell.value, Some("Test Value".to_string()));
        assert!(cell.last_updated <= Utc::now());
    }

    #[test]
    fn test_add_non_consecutive_columns() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Text,
        };
        sheet.add_column(column_a.clone());
        sheet.add_column(column_c.clone());

        assert_eq!(sheet.columns.len(), 2);
        assert_eq!(sheet.columns[&0], column_a);
        assert_eq!(sheet.columns[&2], column_c);
    }

    #[test]
    fn test_formula_evaluation() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A1+B1".to_string()),
        };
        sheet.add_column(column_a);
        sheet.add_column(column_b);
        sheet.add_column(column_c);

        let workflow_job_creator = MockWorkflowJobCreator;
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), &workflow_job_creator)
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 2, &workflow_job_creator);

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("HelloWorld".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 2, &workflow_job_creator);

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("ByeWorld".to_string()));
    }

    #[test]
    fn test_formula_evaluation_multiple_references() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_d = ColumnDefinition {
            id: 3,
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=A1+B1+C1".to_string()),
        };
        sheet.add_column(column_a);
        sheet.add_column(column_b);
        sheet.add_column(column_c);
        sheet.add_column(column_d);

        let workflow_job_creator = MockWorkflowJobCreator;
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), &workflow_job_creator)
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), &workflow_job_creator)
            .unwrap();
        sheet
            .set_cell_value(0, 2, "Again".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 3, &workflow_job_creator);

        let cell = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell.value, Some("HelloWorldAgain".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 3, &workflow_job_creator);

        let cell = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell.value, Some("ByeWorldAgain".to_string()));
    }

    #[test]
    fn test_formula_evaluation_with_literals() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A1+\" \"+B1".to_string()),
        };
        sheet.add_column(column_a);
        sheet.add_column(column_b);
        sheet.add_column(column_c);

        let workflow_job_creator = MockWorkflowJobCreator;
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), &workflow_job_creator)
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 2, &workflow_job_creator);

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("Hello World".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 2, &workflow_job_creator);

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("Bye World".to_string()));
    }

    #[test]
    fn test_dependencies_update() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A1+B1+\"hey\"".to_string()),
        };
        sheet.add_column(column_a);
        sheet.add_column(column_b);
        sheet.add_column(column_c);

        let workflow_job_creator = MockWorkflowJobCreator;
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), &workflow_job_creator)
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), &workflow_job_creator)
            .unwrap();
        sheet.update_cell(0, 2, &workflow_job_creator);

        let dependents = sheet.column_dependency_manager.get_dependents(2);
        assert!(dependents.contains(&0));
        assert!(dependents.contains(&1));

        let reverse_dependents_a = sheet.column_dependency_manager.get_reverse_dependents(0);
        let reverse_dependents_b = sheet.column_dependency_manager.get_reverse_dependents(1);
        assert!(reverse_dependents_a.contains(&2));
        assert!(reverse_dependents_b.contains(&2));
    }
}
