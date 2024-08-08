use chrono::Utc;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use shinkai_message_primitives::schemas::sheet::{ColumnBehavior, ColumnDefinition, UuidString};
    use shinkai_sheet::sheet::Sheet;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn test_add_column() {
        let mut sheet = Sheet::new();
        let column_id = Uuid::new_v4().to_string();
        let column = ColumnDefinition {
            id: column_id.clone(),
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let _ = sheet.set_column(column.clone()).await;
        assert_eq!(sheet.columns.len(), 1);
        assert_eq!(sheet.columns[&column_id], column);
    }

    #[tokio::test]
    async fn test_update_column() {
        let mut sheet = Sheet::new();
        let column_id = Uuid::new_v4().to_string();
        let column_text = ColumnDefinition {
            id: column_id.clone(),
            name: "Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let _ = sheet.set_column(column_text.clone()).await;
        assert_eq!(sheet.columns[&column_id].name, "Text Column");

        let updated_column_text = ColumnDefinition {
            id: column_id.clone(),
            name: "Updated Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let _ = sheet.set_column(updated_column_text.clone()).await;
        assert_eq!(sheet.columns[&column_id].name, "Updated Text Column");
    }

    #[tokio::test]
    async fn test_set_cell_value() {
        let mut sheet = Sheet::new();
        let column_id = Uuid::new_v4().to_string();
        let row_id = Uuid::new_v4().to_string();
        let column = ColumnDefinition {
            id: column_id.clone(),
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let _ = sheet.set_column(column).await;

        // Add a new row before setting the cell value
        let _ = sheet.add_row(row_id.clone()).await;

        let result = sheet
            .set_cell_value(row_id.clone(), column_id.clone(), "Test Value".to_string())
            .await;
        assert!(result.is_ok());

        let cell = sheet.get_cell(row_id.clone(), column_id.clone()).unwrap();
        assert_eq!(cell.value, Some("Test Value".to_string()));
        assert!(cell.last_updated <= Utc::now());
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_add_a_new_column() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_c.clone()).await;

        assert_eq!(sheet.columns.len(), 2);
        assert_eq!(sheet.columns[&column_a_id], column_a);
        assert_eq!(sheet.columns[&column_c_id], column_c);
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_formula_evaluation() {
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
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Ensure the row is created before setting the cell values
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell.value, Some("HelloWorld".to_string()));

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Bye".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell.value, Some("ByeWorld".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_multiple_references() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let column_d_id = Uuid::new_v4().to_string();
        let row_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_d = ColumnDefinition {
            id: column_d_id.clone(),
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=A+B+C".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;
        let _ = sheet.set_column(column_d).await;

        // Ensure the row is created before setting the cell values
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_id.clone(), column_c_id.clone(), "Again".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_d_id.clone()).unwrap();
        assert_eq!(cell.value, Some("HelloWorldAgain".to_string()));

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Bye".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_d_id.clone()).unwrap();
        assert_eq!(cell.value, Some("ByeWorldAgain".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_with_literals() {
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
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A+\" space \"+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Ensure the row is created before setting the cell values
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell.value, Some("Hello space World".to_string()));

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Bye".to_string())
            .await
            .unwrap();

        let cell = sheet.get_cell(row_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell.value, Some("Bye space World".to_string()));
    }

    #[tokio::test]
    async fn test_dependencies_update() {
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
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A+B+\"hey\"".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Ensure the row is created before setting the cell values
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();

        let dependents = sheet.column_dependency_manager.get_dependents(column_c_id.clone());
        assert!(dependents.contains(&column_a_id));
        assert!(dependents.contains(&column_b_id));

        let reverse_dependents_a = sheet
            .column_dependency_manager
            .get_reverse_dependents(column_a_id.clone());
        let reverse_dependents_b = sheet
            .column_dependency_manager
            .get_reverse_dependents(column_b_id.clone());
        assert!(reverse_dependents_a.contains(&column_c_id));
        assert!(reverse_dependents_b.contains(&column_c_id));
    }

    #[tokio::test]
    async fn test_formula_reads_column_a() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let row_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Formula("=A+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;

        // Ensure the row is created before setting the cell values
        sheet.add_row(row_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_id.clone(), column_a_id.clone(), "Not Empty".to_string())
            .await
            .unwrap();

        let cell_b_updated = sheet.get_cell(row_id.clone(), column_b_id.clone()).unwrap();
        assert_eq!(cell_b_updated.value, Some("Not Empty Copy".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_with_literals_and_copy_for_multiple_rows() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let column_d_id = Uuid::new_v4().to_string();
        let row_0_id = Uuid::new_v4().to_string();
        let row_1_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A+\" \"+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Ensure the rows are created before setting the cell values
        sheet.add_row(row_0_id.clone()).await.unwrap();
        sheet.add_row(row_1_id.clone()).await.unwrap();

        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_0_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_a_id.clone(), "Foo".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_b_id.clone(), "Bar".to_string())
            .await
            .unwrap();

        let cell_c_0 = sheet.get_cell(row_0_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_0.value, Some("Hello World".to_string()));

        let cell_c_1 = sheet.get_cell(row_1_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_1.value, Some("Foo Bar".to_string()));

        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Bye".to_string())
            .await
            .unwrap();

        let cell_c_updated_0 = sheet.get_cell(row_0_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_updated_0.value, Some("Bye World".to_string()));

        let column_d = ColumnDefinition {
            id: column_d_id.clone(),
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=C+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_d).await;

        let cell_d_0 = sheet.get_cell(row_0_id.clone(), column_d_id.clone()).unwrap();
        assert_eq!(cell_d_0.value, Some("Bye World Copy".to_string()));

        let cell_d_1 = sheet.get_cell(row_1_id.clone(), column_d_id.clone()).unwrap();
        sheet.print_as_ascii_table();
        assert_eq!(cell_d_1.value, Some("Foo Bar Copy".to_string()));
    }

    #[tokio::test]
    async fn test_remove_columns_with_formula() {
        let mut sheet = Sheet::new();
        let column_a_id = Uuid::new_v4().to_string();
        let column_b_id = Uuid::new_v4().to_string();
        let column_c_id = Uuid::new_v4().to_string();
        let column_d_id = Uuid::new_v4().to_string();
        let row_0_id = Uuid::new_v4().to_string();
        let row_1_id = Uuid::new_v4().to_string();
        let column_a = ColumnDefinition {
            id: column_a_id.clone(),
            name: "Column A".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_b = ColumnDefinition {
            id: column_b_id.clone(),
            name: "Column B".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let column_c = ColumnDefinition {
            id: column_c_id.clone(),
            name: "Column C".to_string(),
            behavior: ColumnBehavior::Formula("=A+\" \"+B".to_string()),
        };

        // Add columns
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Ensure the rows are created before setting the cell values
        sheet.add_row(row_0_id.clone()).await.unwrap();
        sheet.add_row(row_1_id.clone()).await.unwrap();

        // Set cell values
        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Hello".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_0_id.clone(), column_b_id.clone(), "World".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_a_id.clone(), "Foo".to_string())
            .await
            .unwrap();
        sheet
            .set_cell_value(row_1_id.clone(), column_b_id.clone(), "Bar".to_string())
            .await
            .unwrap();

        // Check initial formula results
        let cell_c_0 = sheet.get_cell(row_0_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_0.value, Some("Hello World".to_string()));

        let cell_c_1 = sheet.get_cell(row_1_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_1.value, Some("Foo Bar".to_string()));

        // Update a cell value and check formula result
        sheet
            .set_cell_value(row_0_id.clone(), column_a_id.clone(), "Bye".to_string())
            .await
            .unwrap();
        let cell_c_updated_0 = sheet.get_cell(row_0_id.clone(), column_c_id.clone()).unwrap();
        assert_eq!(cell_c_updated_0.value, Some("Bye World".to_string()));

        // Add another formula column
        let column_d = ColumnDefinition {
            id: column_d_id.clone(),
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=C+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_d).await;

        let cell_d_0 = sheet.get_cell(row_0_id.clone(), column_d_id.clone()).unwrap();
        assert_eq!(cell_d_0.value, Some("Bye World Copy".to_string()));

        let cell_d_1 = sheet.get_cell(row_1_id.clone(), column_d_id.clone()).unwrap();
        assert_eq!(cell_d_1.value, Some("Foo Bar Copy".to_string()));

        // Remove the middle column
        sheet.remove_column(column_b_id.clone()).await.unwrap();
        sheet.print_as_ascii_table();

        assert_eq!(sheet.columns.len(), 3);

        // Remove the first column
        sheet.remove_column(column_a_id.clone()).await.unwrap();
        assert_eq!(sheet.columns.len(), 2);
        sheet.print_as_ascii_table();
    }

    #[tokio::test]
    async fn test_parse_formula_dependencies_text_input() {
        let sheet = Sheet::new();
        let formula = "Say Hello World";
        let dependencies = sheet.parse_formula_dependencies(formula);
        let expected: HashSet<UuidString> = HashSet::new();
        assert_eq!(dependencies, expected);
    }

    #[tokio::test]
    async fn test_parse_formula_dependencies_with_column_reference() {
        let mut sheet = Sheet::new();
        sheet.display_columns.push(Uuid::new_v4().to_string()); // Add a column to display_columns
        let formula = "=A + \" And Space\"";
        let dependencies = sheet.parse_formula_dependencies(formula);
        let mut expected = HashSet::new();
        expected.insert(sheet.display_columns[0].clone()); // Use the UUID from display_columns
        assert_eq!(dependencies, expected);
    }
}
