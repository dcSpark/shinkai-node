use std::any::Any;

use chrono::Utc;
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_sheet::sheet::{CellId, ColumnBehavior, ColumnDefinition, Sheet, WorkflowJobCreator};
use shinkai_sheet::sheet_job::{MockupSheetJob, SheetJob};

struct MockWorkflowJobCreator;

impl WorkflowJobCreator for MockWorkflowJobCreator {
    fn initiate_workflow_job(
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_workflow_job_creator() -> Arc<Mutex<Box<dyn WorkflowJobCreator>>> {
        Arc::new(Mutex::new(Box::new(MockWorkflowJobCreator) as Box<dyn WorkflowJobCreator>))
    }

    #[test]
    fn test_add_column() {
        let mut sheet = Sheet::new();
        let column = ColumnDefinition {
            id: 0,
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        sheet.set_column(column.clone());
        assert_eq!(sheet.columns.len(), 1);
        assert_eq!(sheet.columns[&0], column);
    }

    #[test]
    fn test_update_column() {
        let mut sheet = Sheet::new();
        let column_text = ColumnDefinition {
            id: 0,
            name: "Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        sheet.set_column(column_text.clone());
        assert_eq!(sheet.columns[&0].name, "Text Column");

        let updated_column_text = ColumnDefinition {
            id: 0,
            name: "Updated Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        sheet.set_column(updated_column_text.clone());
        assert_eq!(sheet.columns[&0].name, "Updated Text Column");
    }

    #[tokio::test]
    async fn test_set_cell_value() {
        let mut sheet = Sheet::new();
        let column = ColumnDefinition {
            id: 0,
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        sheet.set_column(column);

        let workflow_job_creator = create_workflow_job_creator();
        let result = sheet
            .set_cell_value(0, 0, "Test Value".to_string(), workflow_job_creator)
            .await;
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
        sheet.set_column(column_a.clone());
        sheet.set_column(column_c.clone());

        assert_eq!(sheet.columns.len(), 2);
        assert_eq!(sheet.columns[&0], column_a);
        assert_eq!(sheet.columns[&2], column_c);
    }

    #[tokio::test]
    async fn test_formula_evaluation() {
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
        sheet.set_column(column_a);
        sheet.set_column(column_b);
        sheet.set_column(column_c);

        let workflow_job_creator = create_workflow_job_creator();
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 2, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("HelloWorld".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 2, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("ByeWorld".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_multiple_references() {
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
        sheet.set_column(column_a);
        sheet.set_column(column_b);
        sheet.set_column(column_c);
        sheet.set_column(column_d);

        let workflow_job_creator = create_workflow_job_creator();
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet
            .set_cell_value(0, 2, "Again".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 3, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell.value, Some("HelloWorldAgain".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 3, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell.value, Some("ByeWorldAgain".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_with_literals() {
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
        sheet.set_column(column_a);
        sheet.set_column(column_b);
        sheet.set_column(column_c);

        let workflow_job_creator = create_workflow_job_creator();
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 2, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("Hello World".to_string()));

        sheet
            .set_cell_value(0, 0, "Bye".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 2, workflow_job_creator.clone()).await;

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("Bye World".to_string()));
    }

    #[tokio::test]
    async fn test_dependencies_update() {
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
        sheet.set_column(column_a);
        sheet.set_column(column_b);
        sheet.set_column(column_c);

        let workflow_job_creator = create_workflow_job_creator();
        sheet
            .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet
            .set_cell_value(0, 1, "World".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 2, workflow_job_creator.clone()).await;

        let dependents = sheet.column_dependency_manager.get_dependents(2);
        assert!(dependents.contains(&0));
        assert!(dependents.contains(&1));

        let reverse_dependents_a = sheet.column_dependency_manager.get_reverse_dependents(0);
        let reverse_dependents_b = sheet.column_dependency_manager.get_reverse_dependents(1);
        assert!(reverse_dependents_a.contains(&2));
        assert!(reverse_dependents_b.contains(&2));
    }

    #[tokio::test]
    async fn test_formula_reads_column_a() {
        let mut sheet = Sheet::new();
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A1+\" Copy\"".to_string()),
        };
        sheet.set_column(column_a);
        sheet.set_column(column_b);

        let workflow_job_creator = create_workflow_job_creator();
        sheet.update_cell(0, 1, workflow_job_creator.clone()).await;

        let cell_b = sheet.get_cell(0, 1).unwrap();
        assert_eq!(cell_b.value, Some(" Copy".to_string()));

        sheet
            .set_cell_value(0, 0, "Not Empty".to_string(), workflow_job_creator.clone())
            .await
            .unwrap();
        sheet.update_cell(0, 1, workflow_job_creator.clone()).await;

        let cell_b_updated = sheet.get_cell(0, 1).unwrap();
        assert_eq!(cell_b_updated.value, Some("Not Empty Copy".to_string()));
    }
}
