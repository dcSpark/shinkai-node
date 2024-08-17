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
            sheet.rows.keys().cloned().collect()
        };

        for (row_index, value) in values.iter().enumerate() {
            let row_id = row_ids.get(row_index).ok_or("Row ID not found")?.clone();
            let mut sheet_manager = sheet_manager.lock().await;
            sheet_manager.set_cell_value(&sheet_id, row_id, column_definition.id.clone(), value.clone()).await?;
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
