#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use shinkai_dsl::parser::parse_workflow;
    use shinkai_message_primitives::schemas::sheet::{CellStatus, ColumnBehavior, ColumnDefinition};
    use shinkai_sheet::sheet::Sheet;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_llm_call_column() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let row_0_id = Uuid::new_v4().to_string();
        let column_text_id = Uuid::new_v4().to_string();
        let column_llm_id = Uuid::new_v4().to_string();
        let column_text = ColumnDefinition {
            id: column_text_id.clone(),
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
            id: column_llm_id.clone(),
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
        assert_eq!(sheet.lock().await.columns[&column_text_id], column_text);
        assert_eq!(sheet.lock().await.columns[&column_llm_id], column_llm);

        sheet.lock().await.add_row(row_0_id.clone()).await.unwrap();

        // Check the value of the cell after the update
        let cell_value = sheet
            .lock()
            .await
            .get_cell_value(Uuid::new_v4().to_string(), column_llm_id.clone());
        assert_eq!(cell_value, None);

        // Simulate job completion
        let jobs = sheet
            .lock()
            .await
            .set_cell_value(row_0_id.clone(), column_llm_id.clone(), "Hello World".to_string())
            .await
            .unwrap();
        assert_eq!(jobs.len(), 0);

        // Check the value of the cell after the job completion
        let cell_value = sheet
            .lock()
            .await
            .get_cell_value(row_0_id.clone(), column_llm_id.clone());
        assert_eq!(cell_value, Some("Hello World".to_string()));

        {
            let sheet_locked = sheet.lock().await;
            let cell_status = sheet_locked
                .get_cell(row_0_id.clone(), column_llm_id)
                .map(|cell| &cell.status);
            assert_eq!(cell_status, Some(&CellStatus::Ready));
            sheet_locked.print_as_ascii_table();
        }
    }

    #[tokio::test]
    async fn test_auto_populate_new_column() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let row_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" Copy\"".to_string()),
        };

        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=B + \" Second Copy\"".to_string()),
        };

        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_b.clone()).await;
        let _ = sheet.set_column(column_c.clone()).await;

        sheet.print_as_ascii_table();

        // Ensure the row is created before setting the cell value
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();

        assert_eq!(
            sheet.get_cell_value(row_id.clone(), column_a_id.clone()),
            Some("Hello".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_id.clone(), column_b_id.clone()),
            Some("Hello Copy".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_id.clone(), column_c_id.clone()),
            Some("Hello Copy Second Copy".to_string())
        );

        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_llm_call_with_dependent_column() {
        let mut sheet = Sheet::new();
        let column_text_id = Uuid::new_v4().to_string();
        let column_llm_id = Uuid::new_v4().to_string();
        let column_formula_id = Uuid::new_v4().to_string();
        let row_id = Uuid::new_v4().to_string();
        let column_text = ColumnDefinition {
            id: column_text_id.clone(),
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
            id: column_llm_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::LLMCall {
                input: "Say Hello World".to_string(),
                workflow,
                llm_provider_name: "MockProvider".to_string(),
                input_hash: None,
            },
        };

        let column_formula = ColumnDefinition {
            id: column_formula_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" And Space\"".to_string()),
        };

        let _text_jobs = sheet.set_column(column_text.clone()).await;
        let _llm_jobs = sheet.set_column(column_llm.clone()).await;
        let _formula_jobs = sheet.set_column(column_formula.clone()).await;

        // Ensure the row is created before setting the cell value
        sheet.add_row(row_id.clone()).await.unwrap();

        // Set value in Column A
        sheet
            .set_cell_value(row_id.clone(), column_text_id.clone(), "Hello".to_string())
            .await
            .unwrap();

        // Check initial state of Column C (formula depending on Column A)
        let cell_value_formula = sheet.get_cell_value(row_id.clone(), column_formula_id.clone());
        assert_eq!(cell_value_formula, Some("Hello And Space".to_string()));

        // Change Column C formula to depend on Column B instead of Column A
        let new_column_formula = ColumnDefinition {
            id: column_formula_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=B + \" Updated\"".to_string()),
        };
        let _ = sheet.set_column(new_column_formula).await;

        // Check Column C value before updating Column B (should be empty or default value)
        let cell_value_formula = sheet.get_cell_value(row_id.clone(), column_formula_id.clone());
        assert_eq!(cell_value_formula, Some(" Updated".to_string()));

        // Simulate LLM call completion for Column B
        sheet
            .set_cell_value(row_id.clone(), column_llm_id.clone(), "Hola Mundo".to_string())
            .await
            .unwrap();

        // Check the value of the LLM call cell (Column B) after the update
        let cell_value_llm = sheet.get_cell_value(row_id.clone(), column_llm_id.clone());
        assert_eq!(cell_value_llm, Some("Hola Mundo".to_string()));

        // Check if Column C reflects the updated value of Column B
        let cell_value_formula = sheet.get_cell_value(row_id.clone(), column_formula_id.clone());
        assert_eq!(cell_value_formula, Some("Hola Mundo Updated".to_string()));

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_multiple_rows() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let row_0_id = Uuid::new_v4().to_string();
        let row_1_id = Uuid::new_v4().to_string();
        let row_2_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
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
            id: column_c_id.clone(),
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

        // Ensure the rows are created before setting the cell values
        sheet.add_row(row_0_id.clone()).await.unwrap();
        sheet.add_row(row_1_id.clone()).await.unwrap();
        sheet.add_row(row_2_id.clone()).await.unwrap();

        // Add data to multiple rows
        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_a_id.clone(), "World".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_2_id.clone(), column_a_id.clone(), "Test".to_string())
            .await
            .unwrap();

        // Check values in Column A and B
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_a_id.clone()),
            Some("Hello".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_b_id.clone()),
            Some("Hello Processed".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_1_id.clone(), column_a_id.clone()),
            Some("World".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_1_id.clone(), column_b_id.clone()),
            Some("World Processed".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_a_id.clone()),
            Some("Test".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_b_id.clone()),
            Some("Test Processed".to_string())
        );

        // Simulate LLM calls on Column C for all rows
        for (i, row_id) in [&row_0_id, &row_1_id, &row_2_id].iter().enumerate() {
            // Simulate job completion for Column C
            sheet
                .set_cell_value(row_id.to_string(), column_c_id.clone(), format!("Summary of row {}", i))
                .await
                .unwrap();
        }

        // Check final values
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_c_id.clone()),
            Some("Summary of row 0".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_1_id.clone(), column_c_id.clone()),
            Some("Summary of row 1".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_c_id.clone()),
            Some("Summary of row 2".to_string())
        );

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_remove_row() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let row_0_id = Uuid::new_v4().to_string();
        let row_1_id = Uuid::new_v4().to_string();
        let row_2_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A + \" Processed\"".to_string()),
        };

        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_b.clone()).await;

        // Ensure the rows are created before setting the cell values
        sheet.add_row(row_0_id.clone()).await.unwrap();
        sheet.add_row(row_1_id.clone()).await.unwrap();
        sheet.add_row(row_2_id.clone()).await.unwrap();

        // Add data to multiple rows
        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_a_id.clone(), "World".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_2_id.clone(), column_a_id.clone(), "Test".to_string())
            .await
            .unwrap();

        // Check initial values
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_a_id.clone()),
            Some("Hello".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_1_id.clone(), column_a_id.clone()),
            Some("World".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_a_id.clone()),
            Some("Test".to_string())
        );

        // Remove a row
        let _ = sheet.remove_row(row_1_id.clone()).await.unwrap();

        // Check values after row removal
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_a_id.clone()),
            Some("Hello".to_string())
        );
        assert_eq!(sheet.get_cell_value(row_1_id.clone(), column_a_id.clone()), None);
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_a_id.clone()),
            Some("Test".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_b_id.clone()),
            Some("Hello Processed".to_string())
        );
        assert_eq!(sheet.get_cell_value(row_1_id.clone(), column_b_id.clone()), None);
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_b_id.clone()),
            Some("Test Processed".to_string())
        );

        // Print final state of the sheet
        sheet.print_as_ascii_table();

        // Add a new row
        let new_row_id = Uuid::new_v4().to_string();
        let _ = sheet.add_row(new_row_id.clone()).await.unwrap();

        // Add a value to the first column of the new row
        sheet
            .set_cell_value(new_row_id.clone(), column_a_id.clone(), "New Value".to_string())
            .await
            .unwrap();

        // Check values after adding a new row
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_a_id.clone()),
            Some("Hello".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(new_row_id.clone(), column_a_id.clone()),
            Some("New Value".to_string())
        ); // New row should have the new value
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_a_id.clone()),
            Some("Test".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(row_0_id.clone(), column_b_id.clone()),
            Some("Hello Processed".to_string())
        );
        assert_eq!(
            sheet.get_cell_value(new_row_id.clone(), column_b_id.clone()),
            Some("New Value Processed".to_string())
        ); // Formula should be recalculated
        assert_eq!(
            sheet.get_cell_value(row_2_id.clone(), column_b_id.clone()),
            Some("Test Processed".to_string())
        );

        // Print final state of the sheet
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_dependency_manager() {
        let sheet = Arc::new(Mutex::new(Sheet::new()));
        let row_id = Uuid::new_v4().to_string();
        let column_text_id = Uuid::new_v4().to_string();
        let column_llm_id = Uuid::new_v4().to_string();
        let column_formula_id = Uuid::new_v4().to_string();

        let column_text = ColumnDefinition {
            id: column_text_id.clone(),
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
            id: column_llm_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::LLMCall {
                input: "=A".to_string(),
                workflow,
                llm_provider_name: "MockProvider".to_string(),
                input_hash: None,
            },
        };

        let column_formula = ColumnDefinition {
            id: column_formula_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=B + \" And Space\"".to_string()),
        };

        {
            let mut sheet = sheet.lock().await;
            let _ = sheet.set_column(column_text.clone()).await;
            let _ = sheet.set_column(column_llm.clone()).await;
            let _ = sheet.set_column(column_formula.clone()).await;
        }

        eprintln!("Checking initial state of the sheet");

        assert_eq!(sheet.lock().await.columns.len(), 3);
        assert_eq!(sheet.lock().await.columns[&column_text_id], column_text);
        assert_eq!(sheet.lock().await.columns[&column_llm_id], column_llm);
        assert_eq!(sheet.lock().await.columns[&column_formula_id], column_formula);

        sheet.lock().await.add_row(row_id.clone()).await.unwrap();

        // Set value in Column A
        sheet
            .lock()
            .await
            .set_cell_value(row_id.clone(), column_text_id.clone(), "Hello".to_string())
            .await
            .unwrap();

        // Check initial state of Column C (formula depending on Column B)
        let cell_value_formula = sheet
            .lock()
            .await
            .get_cell_value(row_id.clone(), column_formula_id.clone());
        assert_eq!(cell_value_formula, Some(" And Space".to_string()));

        eprintln!("Checking initial state of the dependency manager");

        // Simulate LLM call completion for Column B
        sheet
            .lock()
            .await
            .set_cell_value(row_id.clone(), column_llm_id.clone(), "Hola Mundo".to_string())
            .await
            .unwrap();

        // Check the value of the LLM call cell (Column B) after the update
        let cell_value_llm = sheet.lock().await.get_cell_value(row_id.clone(), column_llm_id.clone());
        assert_eq!(cell_value_llm, Some("Hola Mundo".to_string()));

        // Check if Column C reflects the updated value of Column B
        let cell_value_formula = sheet
            .lock()
            .await
            .get_cell_value(row_id.clone(), column_formula_id.clone());
        assert_eq!(cell_value_formula, Some("Hola Mundo And Space".to_string()));

        // Check dependency manager
        {
            let sheet_locked = sheet.lock().await;
            let reverse_dependents_a = sheet_locked
                .column_dependency_manager
                .get_reverse_dependents(column_text_id.clone());
            let reverse_dependents_b = sheet_locked
                .column_dependency_manager
                .get_reverse_dependents(column_llm_id.clone());
            assert!(reverse_dependents_a.contains(&column_llm_id));
            assert!(reverse_dependents_b.contains(&column_formula_id));
        }

        // Update Column C to be a text column
        let new_column_text = ColumnDefinition {
            id: column_formula_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Text,
        };
        eprintln!("Updating Column C to be a text column");
        let _ = sheet.lock().await.set_column(new_column_text.clone()).await;

        // Check the dependencies after updating Column C
        let sheet_locked = sheet.lock().await;
        let dependencies_c = sheet_locked
            .column_dependency_manager
            .dependencies
            .get(&column_formula_id);
        let reverse_dependents_b = sheet_locked
            .column_dependency_manager
            .reverse_dependencies
            .get(&column_llm_id);

        eprintln!("Checking dependencies after updating Column C: {:?}", dependencies_c);
        assert!(dependencies_c.is_none());
        assert!(!reverse_dependents_b.unwrap().contains(&column_formula_id));

        // Print final state of the sheet
        sheet_locked.print_as_ascii_table();
    }
}

// // TODO: add test that A (text missing) -> B (workflow depending on A) -> C (workflo depending on B)
