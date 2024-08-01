use chrono::Utc;
use shinkai_sheet::sheet::{ColumnBehavior, ColumnDefinition, Sheet};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_column() {
        let mut sheet = Sheet::new();
        let column = ColumnDefinition {
            id: 0,
            name: "Column 1".to_string(),
            behavior: ColumnBehavior::Text,
        };
        let _ = sheet.set_column(column.clone()).await;
        assert_eq!(sheet.columns.len(), 1);
        assert_eq!(sheet.columns[&0], column);
    }

    #[tokio::test]
    async fn test_update_column() {
        let mut sheet = Sheet::new();
        let column_text = ColumnDefinition {
            id: 0,
            name: "Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let _ = sheet.set_column(column_text.clone()).await;
        assert_eq!(sheet.columns[&0].name, "Text Column");

        let updated_column_text = ColumnDefinition {
            id: 0,
            name: "Updated Text Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        let _ = sheet.set_column(updated_column_text.clone()).await;
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
        let _ = sheet.set_column(column).await;

        let result = sheet.set_cell_value(0, 0, "Test Value".to_string()).await;
        assert!(result.is_ok());

        let cell = sheet.get_cell(0, 0).unwrap();
        assert_eq!(cell.value, Some("Test Value".to_string()));
        assert!(cell.last_updated <= Utc::now());
    }

    #[tokio::test]
    async fn test_add_non_consecutive_columns() {
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
        let _ = sheet.set_column(column_a.clone()).await;
        let _ = sheet.set_column(column_c.clone()).await;

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
            behavior: ColumnBehavior::Formula("=A+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("HelloWorld".to_string()));

        sheet.set_cell_value(0, 0, "Bye".to_string()).await.unwrap();

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
            behavior: ColumnBehavior::Formula("=A+B+C".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;
        let _ = sheet.set_column(column_d).await;

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();
        sheet.set_cell_value(0, 2, "Again".to_string()).await.unwrap();

        let cell = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell.value, Some("HelloWorldAgain".to_string()));

        sheet.set_cell_value(0, 0, "Bye".to_string()).await.unwrap();

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
            behavior: ColumnBehavior::Formula("=A+\" \"+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();

        let cell = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell.value, Some("Hello World".to_string()));

        sheet.set_cell_value(0, 0, "Bye".to_string()).await.unwrap();

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
            behavior: ColumnBehavior::Formula("=A+B+\"hey\"".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();

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
            behavior: ColumnBehavior::Formula("=A+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;

        let cell_b = sheet.get_cell(0, 1).unwrap();
        assert_eq!(cell_b.value, Some(" Copy".to_string()));

        sheet.set_cell_value(0, 0, "Not Empty".to_string()).await.unwrap();

        let cell_b_updated = sheet.get_cell(0, 1).unwrap();
        assert_eq!(cell_b_updated.value, Some("Not Empty Copy".to_string()));
    }

    #[tokio::test]
    async fn test_formula_evaluation_with_literals_and_copy_for_multiple_rows() {
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
            behavior: ColumnBehavior::Formula("=A+\" \"+B".to_string()),
        };
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();
        sheet.set_cell_value(1, 0, "Foo".to_string()).await.unwrap();
        sheet.set_cell_value(1, 1, "Bar".to_string()).await.unwrap();

        let cell_c_0 = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell_c_0.value, Some("Hello World".to_string()));

        let cell_c_1 = sheet.get_cell(1, 2).unwrap();
        assert_eq!(cell_c_1.value, Some("Foo Bar".to_string()));

        sheet.set_cell_value(0, 0, "Bye".to_string()).await.unwrap();

        let cell_c_updated_0 = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell_c_updated_0.value, Some("Bye World".to_string()));

        let column_d = ColumnDefinition {
            id: 3,
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=C+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_d).await;

        let cell_d_0 = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell_d_0.value, Some("Bye World Copy".to_string()));

        let cell_d_1 = sheet.get_cell(1, 3).unwrap();
        sheet.print_as_ascii_table();
        assert_eq!(cell_d_1.value, Some("Foo Bar Copy".to_string()));
    }

    #[tokio::test]
    async fn test_remove_columns_with_formula() {
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
            behavior: ColumnBehavior::Formula("=A+\" \"+B".to_string()),
        };

        // Add columns
        let _ = sheet.set_column(column_a).await;
        let _ = sheet.set_column(column_b).await;
        let _ = sheet.set_column(column_c).await;

        // Set cell values
        sheet.set_cell_value(0, 0, "Hello".to_string()).await.unwrap();
        sheet.set_cell_value(0, 1, "World".to_string()).await.unwrap();
        sheet.set_cell_value(1, 0, "Foo".to_string()).await.unwrap();
        sheet.set_cell_value(1, 1, "Bar".to_string()).await.unwrap();

        // Check initial formula results
        let cell_c_0 = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell_c_0.value, Some("Hello World".to_string()));

        let cell_c_1 = sheet.get_cell(1, 2).unwrap();
        assert_eq!(cell_c_1.value, Some("Foo Bar".to_string()));

        // Update a cell value and check formula result
        sheet.set_cell_value(0, 0, "Bye".to_string()).await.unwrap();
        let cell_c_updated_0 = sheet.get_cell(0, 2).unwrap();
        assert_eq!(cell_c_updated_0.value, Some("Bye World".to_string()));

        // Add another formula column
        let column_d = ColumnDefinition {
            id: 3,
            name: "Column D".to_string(),
            behavior: ColumnBehavior::Formula("=C+\" Copy\"".to_string()),
        };
        let _ = sheet.set_column(column_d).await;

        let cell_d_0 = sheet.get_cell(0, 3).unwrap();
        assert_eq!(cell_d_0.value, Some("Bye World Copy".to_string()));

        let cell_d_1 = sheet.get_cell(1, 3).unwrap();
        assert_eq!(cell_d_1.value, Some("Foo Bar Copy".to_string()));

        // Remove the middle column
        sheet.remove_column(1).await.unwrap();
        sheet.print_as_ascii_table();

        assert_eq!(sheet.columns.len(), 3);

        // Remove the first column
        sheet.remove_column(0).await.unwrap();
        assert_eq!(sheet.columns.len(), 2);
        sheet.print_as_ascii_table();
    }
}
