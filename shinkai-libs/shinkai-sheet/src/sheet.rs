use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::sheet_job::SheetJob;

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
        prompt_template: String,
        input_columns: Vec<usize>,
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
    rows: HashMap<usize, Vec<Cell>>,
    // Adjacency List: cell -> cells it depends on
    dependencies: HashMap<CellId, HashSet<CellId>>,
    // Reverse Adjacency List: cell -> cells that depend on it
    reverse_dependencies: HashMap<CellId, HashSet<CellId>>,
}

pub trait WorkflowJobCreator {
    fn create_workflow_job(
        &self,
        row: usize,
        col: usize,
        prompt_template: &str,
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
            dependencies: HashMap::new(),
            reverse_dependencies: HashMap::new(),
        }
    }

    pub fn add_column(&mut self, definition: ColumnDefinition) {
        self.columns.insert(definition.id, definition);
    }

    fn set_cell_value(
        &mut self,
        row: usize,
        col: usize,
        value: String,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Result<(), String> {
        if !self.columns.contains_key(&col) {
            return Err("Column index out of bounds".to_string());
        }

        let row_cells = self.rows.entry(row).or_default();
        while row_cells.len() <= col {
            row_cells.push(Cell {
                value: None,
                last_updated: Utc::now(),
            });
        }

        row_cells[col] = Cell {
            value: Some(value),
            last_updated: Utc::now(),
        };

        let changed_cell_id = CellId(format!("{}:{}", row, col));
        self.update_dependent_cells(&changed_cell_id, workflow_job_creator);
        Ok(())
    }

    fn update_dependent_cells(&mut self, changed_cell_id: &CellId, workflow_job_creator: &dyn WorkflowJobCreator) {
        let mut cells_to_update = vec![changed_cell_id.clone()];

        while let Some(cell_id) = cells_to_update.pop() {
            let dependents = self.reverse_dependencies.get(&cell_id).cloned().unwrap_or_default();

            for dependent in dependents {
                let (row, col) = self.cell_id_to_indices(&dependent);
                let jobs = self.update_cell(row, col, workflow_job_creator);
                cells_to_update.push(dependent);
                // Handle jobs if necessary
            }
        }
    }

    fn update_cell(
        &mut self,
        row: usize,
        col: usize,
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Vec<Box<dyn SheetJob>> {
        let mut jobs = Vec::new();
        let column_behavior = self.columns.get(&col).unwrap().behavior.clone();
        match column_behavior {
            ColumnBehavior::Formula(formula) => {
                // Process formula
                // This is a placeholder. You'll need to implement formula processing logic.
                println!("Processing formula: {} for cell {}:{}", formula, row, col);
            }
            ColumnBehavior::LLMCall {
                prompt_template,
                input_columns,
            } => {
                let job = self.create_workflow_job(row, col, &prompt_template, &input_columns, workflow_job_creator);
                jobs.push(job);
            }
            _ => {}
        }
        jobs
    }

    fn create_workflow_job(
        &mut self,
        row: usize,
        col: usize,
        prompt_template: &str,
        input_columns: &[usize],
        workflow_job_creator: &dyn WorkflowJobCreator,
    ) -> Box<dyn SheetJob> {
        let cell_id = CellId(format!("{}:{}", row, col));
        let input_values: Vec<String> = input_columns
            .iter()
            .filter_map(|&col_id| self.get_cell_value(row, col_id))
            .collect();

        let dependencies: Vec<CellId> = input_columns
            .iter()
            .map(|&col_id| CellId(format!("{}:{}", row, col_id)))
            .collect();

        // Update dependency graph
        self.dependencies
            .entry(cell_id.clone())
            .or_default()
            .extend(dependencies.iter().cloned());
        for dep in &dependencies {
            self.reverse_dependencies
                .entry(dep.clone())
                .or_default()
                .insert(cell_id.clone());
        }

        workflow_job_creator.create_workflow_job(row, col, prompt_template, input_columns, &input_values)
    }

    fn get_column_index(&self, column_id: usize) -> usize {
        *self.columns.get(&column_id).map(|col| &col.id).unwrap_or(&0)
    }

    fn get_cell_value(&self, row: usize, col: usize) -> Option<String> {
        self.rows
            .get(&row)
            .and_then(|row_cells| row_cells.get(col))
            .and_then(|cell| cell.value.clone())
    }

    fn fill_prompt_template(&self, template: &str, values: &[String]) -> String {
        let mut result = template.to_string();
        for (i, value) in values.iter().enumerate() {
            result = result.replace(&format!("{{${}}}", i + 1), value);
        }
        result
    }

    fn cell_id_to_indices(&self, cell_id: &CellId) -> (usize, usize) {
        let parts: Vec<&str> = cell_id.0.split(':').collect();
        (parts[0].parse().unwrap(), parts[1].parse().unwrap())
    }

    fn remove_dependency(&mut self, from: &CellId, to: &CellId) {
        if let Some(deps) = self.dependencies.get_mut(from) {
            deps.remove(to);
        }
        if let Some(rev_deps) = self.reverse_dependencies.get_mut(to) {
            rev_deps.remove(from);
        }
    }

    fn add_dependency(&mut self, from: &CellId, to: &CellId) {
        self.dependencies.entry(from.clone()).or_default().insert(to.clone());
        self.reverse_dependencies
            .entry(to.clone())
            .or_default()
            .insert(from.clone());
    }

    // Additional helper methods

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.rows.get(&row).and_then(|row_cells| row_cells.get(col))
    }

    pub fn get_column_definitions(&self) -> Vec<(usize, &ColumnDefinition)> {
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
            prompt_template: &str,
            input_columns: &[usize],
            cell_values: &[String],
        ) -> Box<dyn SheetJob> {
            Box::new(MockupSheetJob::new(
                "mock_job_id".to_string(),
                CellId(format!("{}:{}", row, col)),
                prompt_template.to_string(),
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
}