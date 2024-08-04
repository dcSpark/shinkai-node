#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use shinkai_dsl::parser::parse_workflow;
    use shinkai_message_primitives::schemas::sheet::{CellStatus, ColumnBehavior, ColumnDefinition};
    use shinkai_sheet::sheet::Sheet;
    use tokio::sync::Mutex;

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
                input_hash: None,
            },
        };

        {
            let mut sheet = sheet.lock().await;
            let _ = sheet.set_column(column_text.clone()).await;
            let _ = sheet.set_column(column_llm.clone()).await;
        }

        assert_eq!(sheet.lock().await.columns.len(), 2);
        assert_eq!(sheet.lock().await.columns[&0], column_text);
        assert_eq!(sheet.lock().await.columns[&1], column_llm);

        // Check the value of the cell after the update
        let cell_value = sheet.lock().await.get_cell_value(0, 1);
        assert_eq!(cell_value, None);

        // TODO: re-enable and fix
        // {
        //     let sheet_locked = sheet.lock().await;
        //     let cell_status = sheet_locked.get_cell(0, 1).map(|cell| &cell.status);
        //     assert_eq!(cell_status, Some(&CellStatus::Pending));
        // }

        // Simulate job completion
        let jobs = sheet
            .lock()
            .await
            .set_cell_value(0, 1, "Hello World".to_string())
            .await
            .unwrap();
        assert_eq!(jobs.len(), 0);

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
        let mut sheet = Sheet::new();
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

        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_b.clone()).await;
        let _ = sheet.set_column(column_c.clone()).await;

        sheet.print_as_ascii_table();

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();

        assert_eq!(sheet.get_cell_value(0, 0), Some("Hello".to_string()));
        assert_eq!(sheet.get_cell_value(0, 1), Some("Hello Copy".to_string()));
        assert_eq!(sheet.get_cell_value(0, 2), Some("Hello Copy Second Copy".to_string()));

        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_llm_call_with_dependent_column() {
        let mut sheet = Sheet::new();
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
                input_hash: None,
            },
        };

        let column_formula = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" And Space\"".to_string()),
        };

        let _text_jobs = sheet.set_column(column_text.clone()).await;
        let llm_jobs = sheet.set_column(column_llm.clone()).await;
        let _formula_jobs = sheet.set_column(column_formula.clone()).await;

        assert_eq!(llm_jobs.as_ref().unwrap().len(), 1);

        // Set value in Column A
        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();

        // Check initial state of Column C (formula depending on Column A)
        let cell_value_formula = sheet.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some("Hello And Space".to_string()));

        // Change Column C formula to depend on Column B instead of Column A
        let new_column_formula = ColumnDefinition {
            id: 2,
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=B + \" Updated\"".to_string()),
        };
        let _ = sheet.set_column(new_column_formula).await;

        // Check Column C value before updating Column B (should be empty or default value)
        let cell_value_formula = sheet.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some(" Updated".to_string()));

        // Simulate LLM call completion for Column B
        sheet.set_cell_value(0, 1, "Hola Mundo".to_string()).await.unwrap();

        // Check the value of the LLM call cell (Column B) after the update
        let cell_value_llm = sheet.get_cell_value(0, 1);
        assert_eq!(cell_value_llm, Some("Hola Mundo".to_string()));

        // Check if Column C reflects the updated value of Column B
        let cell_value_formula = sheet.get_cell_value(0, 2);
        assert_eq!(cell_value_formula, Some("Hola Mundo Updated".to_string()));

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_multiple_rows() {
        let mut sheet = Sheet::new();
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
                input_hash: None,
            },
        };

        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_b.clone()).await;
        let _ = sheet.set_column(column_c.clone()).await;

        // Add data to multiple rows
        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(1, 0, "World".to_string()).await.unwrap();
        sheet.set_cell_value(2, 0, "Test".to_string()).await.unwrap();

        // Check values in Column A and B
        assert_eq!(sheet.get_cell_value(0, 0), Some("Hello".to_string()));
        assert_eq!(sheet.get_cell_value(0, 1), Some("Hello Processed".to_string()));
        assert_eq!(sheet.get_cell_value(1, 0), Some("World".to_string()));
        assert_eq!(sheet.get_cell_value(1, 1), Some("World Processed".to_string()));
        assert_eq!(sheet.get_cell_value(2, 0), Some("Test".to_string()));
        assert_eq!(sheet.get_cell_value(2, 1), Some("Test Processed".to_string()));

        // Simulate LLM calls on Column C for all rows
        for row in 0..3 {
            // Simulate job completion for Column C
            sheet
                .set_cell_value(row, 2, format!("Summary of row {}", row))
                .await
                .unwrap();
        }

        // Check final values
        assert_eq!(sheet.get_cell_value(0, 2), Some("Summary of row 0".to_string()));
        assert_eq!(sheet.get_cell_value(1, 2), Some("Summary of row 1".to_string()));
        assert_eq!(sheet.get_cell_value(2, 2), Some("Summary of row 2".to_string()));

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_remove_row() {
        let mut sheet = Sheet::new();
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

        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_b.clone()).await;

        // Add data to multiple rows
        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(1, 0, "World".to_string()).await.unwrap();
        sheet.set_cell_value(2, 0, "Test".to_string()).await.unwrap();

        // Check initial values
        assert_eq!(sheet.get_cell_value(0, 0), Some("Hello".to_string()));
        assert_eq!(sheet.get_cell_value(1, 0), Some("World".to_string()));
        assert_eq!(sheet.get_cell_value(2, 0), Some("Test".to_string()));

        // Remove a row
        let _ = sheet.remove_row(1).await.unwrap();

        // Check values after row removal
        assert_eq!(sheet.get_cell_value(0, 0), Some("Hello".to_string()));
        assert_eq!(sheet.get_cell_value(1, 0), None);
        assert_eq!(sheet.get_cell_value(2, 0), Some("Test".to_string()));
        assert_eq!(sheet.get_cell_value(0, 1), Some("Hello Processed".to_string()));
        assert_eq!(sheet.get_cell_value(1, 1), None);
        assert_eq!(sheet.get_cell_value(2, 1), Some("Test Processed".to_string()));

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }
}

// TODO: add test that A (text missing) -> B (workflow depending on A) -> C (workflo depending on B)
