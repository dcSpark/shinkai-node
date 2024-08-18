use std::{collections::HashMap, pin::Pin, sync::Arc};

use crate::{
    managers::sheet_manager::SheetManager,
    tools::{argument::ToolArgument, rust_tools::RustTool, shinkai_tool::ShinkaiTool},
};
use shinkai_message_primitives::schemas::sheet::{ColumnBehavior, ColumnDefinition};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct SheetRustFunctions;

impl SheetRustFunctions {
    pub async fn create_new_column_with_values(
        sheet_manager: Arc<Mutex<SheetManager>>,
        sheet_id: String,
        values: String,
    ) -> Result<String, String> {
        // Split the values string into a Vec<String>
        let values: Vec<String> = values.split(',').map(|s| s.trim().to_string()).collect();

        // Create a new column of type Text
        let column_definition = ColumnDefinition {
            id: Uuid::new_v4().to_string(),
            name: "New Column".to_string(),
            behavior: ColumnBehavior::Text,
        };

        // Set the new column
        {
            let mut sheet_manager = sheet_manager.lock().await;
            sheet_manager.set_column(&sheet_id, column_definition.clone()).await?;
        }

        // Ensure the number of rows matches the number of values
        {
            let mut sheet_manager = sheet_manager.lock().await;
            while {
                let (sheet, _) = sheet_manager.sheets.get_mut(&sheet_id).ok_or("Sheet ID not found")?;
                sheet.rows.len() < values.len()
            } {
                sheet_manager.add_row(&sheet_id, None).await?;
            }
        }

        // Set values for the new column
        let row_ids: Vec<String> = {
            let sheet_manager = sheet_manager.lock().await;
            let (sheet, _) = sheet_manager.sheets.get(&sheet_id).ok_or("Sheet ID not found")?;
            sheet.display_rows.clone()
        };

        for (row_index, value) in values.iter().enumerate() {
            let row_id = row_ids.get(row_index).ok_or("Row ID not found")?.clone();
            let mut sheet_manager = sheet_manager.lock().await;
            sheet_manager
                .set_cell_value(&sheet_id, row_id, column_definition.id.clone(), value.clone())
                .await?;
        }

        Ok("Column created successfully".to_string())
    }

    fn get_tool_map() -> HashMap<
        &'static str,
        fn(
            Arc<Mutex<SheetManager>>,
            String,
            String,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>,
    > {
        let mut tool_map: HashMap<
            &str,
            fn(
                Arc<Mutex<SheetManager>>,
                String,
                String,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>,
        > = HashMap::new();
        tool_map.insert("create_new_column_with_values", |sheet_manager, sheet_id, values| {
            Box::pin(SheetRustFunctions::create_new_column_with_values(
                sheet_manager,
                sheet_id,
                values,
            ))
        });
        tool_map
    }

    pub fn get_tool_function(
        name: String,
    ) -> Option<
        fn(
            Arc<Mutex<SheetManager>>,
            String,
            String,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>,
    > {
        let tool_map = Self::get_tool_map();
        tool_map.get(name.as_str()).copied()
    }

    pub fn sheet_rust_fn() -> Vec<ShinkaiTool> {
        // Add the tool definition for create_new_column_with_values
        let create_new_column_tool = RustTool::new(
            "create_new_column_with_values".to_string(),
            "Creates a new column with the provided values. Values should be separated by commas. Example: 'value1, value2, value3'".to_string(),
            vec![
                ToolArgument::new(
                    "values".to_string(),
                    "string".to_string(),
                    "The values to populate the new column, separated by commas".to_string(),
                    true,
                ),
            ],
            None,
        );
        let shinkai_tool = ShinkaiTool::Rust(create_new_column_tool, true);

        vec![shinkai_tool]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::ShinkaiDB, llm_provider::job_manager::JobManagerTrait, network::ws_manager::WSUpdateHandler};
    use async_trait::async_trait;
    use futures::Future;
    use shinkai_message_primitives::{
        schemas::shinkai_name::ShinkaiName,
        shinkai_message::shinkai_message_schemas::{JobCreationInfo, JobMessage},
    };
    use shinkai_vector_resources::utils::hash_string;
    use std::{fs, path::Path, sync::Arc};
    use tokio::sync::Mutex;

    struct MockJobManager;

    #[async_trait]
    impl JobManagerTrait for MockJobManager {
        fn create_job<'a>(
            &'a mut self,
            _job_creation_info: JobCreationInfo,
            _user_profile: &'a ShinkaiName,
            _agent_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
            Box::pin(async move { Ok("mock_job_id".to_string()) })
        }

        fn queue_job_message<'a>(
            &'a mut self,
            _job_message: &'a JobMessage,
            _user_profile: &'a ShinkaiName,
        ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
            Box::pin(async move { Ok("mock_job_id".to_string()) })
        }
    }

    pub fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);

        let lance_path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(lance_path);
    }

    pub fn create_testing_db(node_name: String) -> ShinkaiDB {
        let db_path = format!("db_tests/{}", hash_string(&node_name));
        ShinkaiDB::new(&db_path).unwrap()
    }

    #[tokio::test]
    async fn test_set_column_with_mock_job_manager() {
        setup();
        let node_name = "@@test.arb-sep-shinkai".to_string();
        let db = create_testing_db(node_name.clone());
        let db = Arc::new(db);
        let node_name = ShinkaiName::new(node_name).unwrap();
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        let sheet_manager = Arc::new(Mutex::new(
            SheetManager::new(Arc::downgrade(&db), node_name, ws_manager)
                .await
                .unwrap(),
        ));

        let mock_job_manager = Arc::new(Mutex::new(MockJobManager));
        sheet_manager.lock().await.set_job_manager(mock_job_manager);

        let sheet_id = sheet_manager.lock().await.create_empty_sheet().unwrap();

        // Call create_new_column_with_values with the values: "USA, Chile, Canada"
        let result = SheetRustFunctions::create_new_column_with_values(
            sheet_manager.clone(),
            sheet_id.clone(),
            "USA, Chile, Canada".to_string(),
        )
        .await;
        assert!(result.is_ok(), "Creating new column with values should succeed");

        {
            let sheet_manager = sheet_manager.lock().await;
            let sheet = sheet_manager.get_sheet(&sheet_id).unwrap();
            assert_eq!(sheet.columns.len(), 1, "There should be one column in the sheet");
            assert_eq!(sheet.rows.len(), 3, "There should be three rows in the sheet");

            // Check the order of the first column
            let col_id = sheet.display_columns.get(0).expect("Column ID not found").clone();
            let expected_values = vec!["USA", "Chile", "Canada"];
            for (i, expected_value) in expected_values.iter().enumerate() {
                let row_id = sheet.display_rows.get(i).expect("Row ID not found").clone();
                let cell_value = sheet
                    .get_cell_value(row_id.clone(), col_id.clone())
                    .expect("Cell value not found");
                assert_eq!(
                    cell_value, *expected_value,
                    "The value in row {} of the first column should be '{}'",
                    i, expected_value
                );
            }
        }

        // Call create_new_column_with_values again with the value: "Italy"
        let result = SheetRustFunctions::create_new_column_with_values(
            sheet_manager.clone(),
            sheet_id.clone(),
            "Italy".to_string(),
        )
        .await;
        assert!(result.is_ok(), "Creating new column with a single value should succeed");

        {
            let sheet_manager = sheet_manager.lock().await;
            let sheet = sheet_manager.get_sheet(&sheet_id).unwrap();
            assert_eq!(sheet.columns.len(), 2, "There should be two columns in the sheet");
            assert_eq!(sheet.rows.len(), 3, "There should still be three rows in the sheet");

            // Check that "Italy" is in the first row, second column
            let row_id = sheet.display_rows.get(0).expect("Row ID not found").clone();
            let col_id = sheet.display_columns.get(1).expect("Column ID not found").clone();

            let cell_value = sheet
                .get_cell_value(row_id.clone(), col_id.clone())
                .expect("Cell value not found");
            assert_eq!(
                cell_value, "Italy",
                "The value in the first row, second column should be 'Italy'"
            );
        }
    }
}
