use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_sheet::sheet::{CellId, ColumnBehavior, ColumnDefinition, Sheet, WorkflowJobCreator};
use shinkai_sheet::sheet_job::{MockupSheetJob, SheetJob};
use std::any::Any;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use shinkai_dsl::parser::parse_workflow;
    use shinkai_sheet::sheet::CellStatus;
    use tokio::sync::Mutex;

    use super::*;

    #[derive(Clone)]
    struct MockWorkflowJobCreator {
        sheet: Arc<Mutex<Sheet>>,
        workflow_job_creator: Option<Arc<Mutex<Box<dyn WorkflowJobCreator>>>>,
    }

    impl MockWorkflowJobCreator {
        fn new(sheet: Arc<Mutex<Sheet>>) -> Self {
            Self {
                sheet,
                workflow_job_creator: None,
            }
        }

        fn set_workflow_job_creator(&mut self, creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>) {
            self.workflow_job_creator = Some(creator);
        }

        async fn complete_job(&self, cell_id: CellId, result: String) {
            let (row, col) = {
                let sheet = self.sheet.lock().await;
                sheet.cell_id_to_indices(&cell_id)
            };

            let mut sheet = self.sheet.lock().await;
            if let Some(ref workflow_job_creator) = self.workflow_job_creator {
                sheet
                    .set_cell_value(row, col, result, workflow_job_creator.clone())
                    .await
                    .expect("Failed to set cell value");
            }
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
            let workflow_job_creator = self.workflow_job_creator.clone().unwrap();
            tokio::spawn(async move {
                // Simulate job completion
                let creator = MockWorkflowJobCreator {
                    sheet,
                    workflow_job_creator: Some(workflow_job_creator),
                };
                creator.complete_job(cell_id, result).await;
            });

            Box::new(MockupSheetJob::new(
                "mock_job_id".to_string(),
                cell_id_clone,
                "".to_string(),
                input_columns.iter().map(|&col| CellId(col.to_string())).collect(),
            ))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    impl MockWorkflowJobCreator {
        async fn complete_job_from_trait_object(
            workflow_job_creator: Arc<Mutex<Box<dyn WorkflowJobCreator>>>,
            _sheet: Arc<Mutex<Sheet>>,
            cell_id: CellId,
            result: String,
        ) {
            let creator = workflow_job_creator.lock().await;
            if let Some(mock_creator) = creator.as_any().downcast_ref::<MockWorkflowJobCreator>() {
                mock_creator.complete_job(cell_id, result).await;
            } else {
                panic!("Failed to downcast to MockWorkflowJobCreator");
            }
        }

        fn create_with_workflow_job_creator(sheet: Arc<Mutex<Sheet>>) -> Arc<Mutex<Box<dyn WorkflowJobCreator>>> {
            let mut mock_creator = MockWorkflowJobCreator::new(sheet.clone());
            let workflow_job_creator =
                Arc::new(Mutex::new(Box::new(mock_creator.clone()) as Box<dyn WorkflowJobCreator>));
            mock_creator.set_workflow_job_creator(workflow_job_creator.clone());
            Arc::new(Mutex::new(Box::new(mock_creator) as Box<dyn WorkflowJobCreator>))
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

        let workflow_job_creator = MockWorkflowJobCreator::create_with_workflow_job_creator(sheet.clone());
        let jobs = sheet.lock().await.update_cell(0, 1, workflow_job_creator.clone()).await;

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
        {
            MockWorkflowJobCreator::complete_job_from_trait_object(
                workflow_job_creator,
                sheet.clone(),
                CellId("0:1".to_string()),
                "Hello World".to_string(),
            )
            .await;
        }

        // Check the value of the cell after the job completion
        let cell_value = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value, Some("Hello World".to_string()));

        {
            let sheet_locked = sheet.lock().await;
            let cell_status = sheet_locked.get_cell(0, 1).map(|cell| &cell.status);
            assert_eq!(cell_status, Some(&CellStatus::Ready));
            sheet_locked.print_as_ascii_table();
        }
    }

    #[tokio::test]
    async fn test_auto_populate_new_column() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" Copy\"".to_string()),
        };

        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=B + \" Second Copy\"".to_string()),
        };

        let workflow_job_creator = MockWorkflowJobCreator::create_with_workflow_job_creator(sheet.clone());

        {
            let mut locked_sheet = sheet.lock().await;
            locked_sheet.set_column(column_a.clone());
            locked_sheet.set_column(column_b.clone());
            locked_sheet.set_column(column_c.clone());
            locked_sheet.print_as_ascii_table();
        }

        {
            let mut sheet_locked = sheet.lock().await;
            sheet_locked
                .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
                .await
                .unwrap();
            assert_eq!(sheet_locked.get_cell_value(0, 0), Some("Hello".to_string()));
            assert_eq!(sheet_locked.get_cell_value(0, 1), Some("Hello Copy".to_string()));
            assert_eq!(
                sheet_locked.get_cell_value(0, 2),
                Some("Hello Copy Second Copy".to_string())
            );
            sheet_locked.print_as_ascii_table();
        }
    }

    #[tokio::test]
    async fn test_llm_call_with_dependent_column() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let column_text = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
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
            name: "Column B".to_string(),
            behavior: ColumnBehavior::LLMCall {
                input: "Say Hello World".to_string(),
                workflow,
                llm_provider_name: "MockProvider".to_string(),
            },
        };

        let column_formula = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" And Space\"".to_string()),
        };

        {
            let mut sheet = sheet.lock().await;
            sheet.set_column(column_text.clone());
            sheet.set_column(column_llm.clone());
            sheet.set_column(column_formula.clone());
        }

        let workflow_job_creator = MockWorkflowJobCreator::create_with_workflow_job_creator(sheet.clone());

        // Set value in Column A
        {
            let mut sheet_locked = sheet.lock().await;
            sheet_locked
                .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
                .await
                .unwrap();
        }

        // Check initial state of Column C (formula depending on Column A)
        let cell_value_formula = sheet.lock().await.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some("Hello And Space".to_string()));

        // Change Column C formula to depend on Column B instead of Column A
        {
            let mut sheet_locked = sheet.lock().await;
            let new_column_formula = ColumnDefinition {
                id: 2,
                name: "Column C".to_string(),
                behavior: ColumnBehavior::Formula("=B + \" Updated\"".to_string()),
            };
            sheet_locked.set_column(new_column_formula);
        }

        // Check Column C value before updating Column B (should be empty or default value)
        let cell_value_formula = sheet.lock().await.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some(" Updated".to_string()));

        // Perform LLM call on Column B
        let jobs = sheet.lock().await.update_cell(0, 1, workflow_job_creator.clone()).await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id(), "mock_job_id");

        // Check the value of the LLM call cell (Column B) after the update
        let cell_value_llm = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value_llm, None);

        // Simulate job completion for Column B
        MockWorkflowJobCreator::complete_job_from_trait_object(
            workflow_job_creator.clone(),
            sheet.clone(),
            CellId("0:1".to_string()),
            "Hola Mundo".to_string(),
        )
        .await;

        // Check the value of the LLM call cell (Column B) after the job completion
        let cell_value_llm = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value_llm, Some("Hola Mundo".to_string()));

        // Check if Column C reflects the updated value of Column B
        let cell_value_formula = sheet.lock().await.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some("Hola Mundo Updated".to_string()));

        // Print final state of the sheet
        {
            let sheet_locked = sheet.lock().await;
            sheet_locked.print_as_ascii_table();
        }
    }

    #[tokio::test]
    async fn test_multiple_rows() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let column_a = ColumnDefinition {
            id: 0,
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let column_b = ColumnDefinition {
            id: 1,
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" Processed\"".to_string()),
        };

        let workflow_str = r#"
        workflow WorkflowTest v0.1 {
            step Main {
                $RESULT = call opinionated_inference($INPUT)
            }
        }
        "#;
        let workflow = parse_workflow(workflow_str).unwrap();

        let column_c = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::LLMCall {
                input: "Summarize: $INPUT".to_string(),
                workflow,
                llm_provider_name: "MockProvider".to_string(),
            },
        };

        {
            let mut sheet = sheet.lock().await;
            sheet.set_column(column_a.clone());
            sheet.set_column(column_b.clone());
            sheet.set_column(column_c.clone());
        }

        let workflow_job_creator = MockWorkflowJobCreator::create_with_workflow_job_creator(sheet.clone());

        // Add data to multiple rows
        {
            let mut sheet_locked = sheet.lock().await;
            sheet_locked
                .set_cell_value(0, 0, "Hello".to_string(), workflow_job_creator.clone())
                .await
                .unwrap();
            sheet_locked
                .set_cell_value(1, 0, "World".to_string(), workflow_job_creator.clone())
                .await
                .unwrap();
            sheet_locked
                .set_cell_value(2, 0, "Test".to_string(), workflow_job_creator.clone())
                .await
                .unwrap();
        }

        // Check values in Column A and B
        {
            let sheet_locked = sheet.lock().await;
            assert_eq!(sheet_locked.get_cell_value(0, 0), Some("Hello".to_string()));
            assert_eq!(sheet_locked.get_cell_value(0, 1), Some("Hello Processed".to_string()));
            assert_eq!(sheet_locked.get_cell_value(1, 0), Some("World".to_string()));
            assert_eq!(sheet_locked.get_cell_value(1, 1), Some("World Processed".to_string()));
            assert_eq!(sheet_locked.get_cell_value(2, 0), Some("Test".to_string()));
            assert_eq!(sheet_locked.get_cell_value(2, 1), Some("Test Processed".to_string()));
        }

        // Perform LLM calls on Column C for all rows
        for row in 0..3 {
            let jobs = sheet
                .lock()
                .await
                .update_cell(row, 2, workflow_job_creator.clone())
                .await;
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].id(), "mock_job_id");

            // Simulate job completion for Column C
            MockWorkflowJobCreator::complete_job_from_trait_object(
                workflow_job_creator.clone(),
                sheet.clone(),
                CellId(format!("{}:2", row)),
                format!("Summary of row {}", row),
            )
            .await;
        }

        // Check final values
        {
            let sheet_locked = sheet.lock().await;
            assert_eq!(sheet_locked.get_cell_value(0, 2), Some("Summary of row 0".to_string()));
            assert_eq!(sheet_locked.get_cell_value(1, 2), Some("Summary of row 1".to_string()));
            assert_eq!(sheet_locked.get_cell_value(2, 2), Some("Summary of row 2".to_string()));

            // Print final state of the sheet
            sheet_locked.print_as_ascii_table();
        }
    }
}
