use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fmt, sync::Arc};
use uuid::Uuid;

use crate::sheet_job::SheetJob;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ColumnDefinition {
    id: String,
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
        input_columns: Vec<String>,
    },
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Cell {
    value: Option<String>,
    last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CellId(pub String);

#[derive(Serialize)]
pub struct Sheet {
    columns: Vec<ColumnDefinition>,
    rows: HashMap<usize, Vec<Cell>>,
    jobs: Vec<Box<dyn SheetJob>>,
    // Adjacency List: cell -> cells it depends on
    dependencies: HashMap<CellId, HashSet<CellId>>,
    // Reverse Adjacency List: cell -> cells that depend on it
    reverse_dependencies: HashMap<CellId, HashSet<CellId>>,
    #[serde(skip)]
    workflow_job_creator: Arc<dyn WorkflowJobCreator>,
}

impl fmt::Debug for Sheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sheet")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("jobs", &self.jobs)
            .field("dependencies", &self.dependencies)
            .field("reverse_dependencies", &self.reverse_dependencies)
            .field("workflow_job_creator", &"<dyn WorkflowJobCreator>")
            .finish()
    }
}

pub trait WorkflowJobCreator {
    fn create_workflow_job(
        &self,
        row: usize,
        col: usize,
        prompt_template: &str,
        input_columns: &[String],
        cell_values: &[String],
    ) -> Box<dyn SheetJob>;
}

impl Sheet {
    pub fn new(workflow_job_creator: Box<dyn WorkflowJobCreator>) -> Self {
        Self {
            columns: Vec::new(),
            rows: HashMap::new(),
            jobs: Vec::new(),
            dependencies: HashMap::new(),
            reverse_dependencies: HashMap::new(),
            workflow_job_creator: Arc::clone(&self.workflow_job_creator),
        }
    }

    pub fn add_column(&mut self, definition: ColumnDefinition) {
        self.columns.push(definition);
    }

    pub fn set_cell_value(&mut self, row: usize, col: usize, value: String) -> Result<(), String> {
        if col >= self.columns.len() {
            return Err("Column index out of bounds".to_string());
        }

        let row_cells = self.rows.entry(row).or_insert_with(Vec::new);
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
        self.update_dependent_cells(&changed_cell_id);
        Ok(())
    }

    fn update_dependent_cells(&mut self, changed_cell_id: &CellId) {
        let mut cells_to_update = vec![changed_cell_id.clone()];
    
        while let Some(cell_id) = cells_to_update.pop() {
            let dependents = self.reverse_dependencies.get(&cell_id)
                .cloned()
                .unwrap_or_default();
            
            for dependent in dependents {
                let (row, col) = self.cell_id_to_indices(&dependent);
                self.update_cell(row, col);
                cells_to_update.push(dependent);
            }
        }
    }

    fn update_cell(&mut self, row: usize, col: usize) {
        let column_behavior = self.columns[col].behavior.clone();
        match column_behavior {
            ColumnBehavior::Formula(formula) => {
                // Process formula
                // This is a placeholder. You'll need to implement formula processing logic.
                println!("Processing formula: {} for cell {}:{}", formula, row, col);
            },
            ColumnBehavior::LLMCall { prompt_template, input_columns } => {
                self.create_workflow_job(row, col, &prompt_template, &input_columns);
            },
            _ => {}
        }
    }

    fn create_workflow_job(&mut self, row: usize, col: usize, prompt_template: &str, input_columns: &[String]) {
        let cell_id = CellId(format!("{}:{}", row, col));
        let input_values: Vec<String> = input_columns
            .iter()
            .filter_map(|col_id| self.get_cell_value(row, self.get_column_index(col_id)))
            .collect();
    
        let dependencies: Vec<CellId> = input_columns.iter().map(|col_id| {
            let col_index = self.get_column_index(col_id);
            CellId(format!("{}:{}", row, col_index))
        }).collect();
    
        // Update dependency graph
        self.dependencies.entry(cell_id.clone()).or_default().extend(dependencies.iter().cloned());
        for dep in &dependencies {
            self.reverse_dependencies.entry(dep.clone()).or_default().insert(cell_id.clone());
        }
    
        let job = self.workflow_job_creator.create_workflow_job(row, col, prompt_template, input_columns, &input_values);
        self.jobs.push(job);
    }

    fn get_column_index(&self, column_id: &str) -> usize {
        self.columns.iter().position(|col| col.id == column_id).unwrap_or(0)
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

    pub fn get_jobs(&self) -> &[Box<dyn SheetJob>] {
        &self.jobs
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.rows.get(&row).and_then(|row_cells| row_cells.get(col))
    }

    pub fn get_column_definitions(&self) -> &[ColumnDefinition] {
        &self.columns
    }
}
