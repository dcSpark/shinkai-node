use std::{
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_runner::tools::run_result::RunResult;

use super::error::ToolError;

pub fn convert_to_shinkai_file_protocol(node_name: &ShinkaiName, path: &str, app_id: &str) -> String {
    // Convert the input path to a PathBuf for platform-agnostic manipulation
    let path_buf = PathBuf::from(path);

    // Create the tools_storage/app_id pattern as a PathBuf
    let mut storage_pattern = PathBuf::from("tools_storage");
    storage_pattern.push(app_id);

    // Try to find the tools_storage/app_id part in the path components
    if let Some(position) = path_buf.components().enumerate().find(|(_, component)| {
        let current_path = Path::new(component.as_os_str());
        current_path == Path::new("tools_storage")
    }) {
        // Get all components after "tools_storage"
        if let Some(relative_path) = path_buf
            .components()
            .skip(position.0)
            .skip(2)
            .collect::<PathBuf>()
            .to_str()
        {
            // Construct the shinkai URL with forward slashes regardless of platform
            return format!(
                "shinkai://file/{}/{}/{}",
                node_name,
                app_id,
                relative_path.replace('\\', "/")
            );
        }
    }

    println!("[Running DenoTool] Failed to convert to shinkai file protocol {}", path);
    "".to_string()
}

fn get_files_in_directories(directories: Vec<&PathBuf>) -> io::Result<Vec<DirEntry>> {
    let mut files = Vec::new();

    for directory in directories {
        let entries = fs::read_dir(directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                files.push(entry);
            } else if path.is_dir() {
                // Recursively get files from subdirectories
                let sub_files = get_files_in_directories(vec![&path])?;
                files.extend(sub_files);
            }
        }
    }

    Ok(files)
}

fn get_files_after(start_time: u64, files: Vec<DirEntry>) -> Vec<(String, u64)> {
    files
        .iter()
        .map(|file| {
            let name = file.path().to_str().unwrap_or_default().to_string();
            let modified = file
                .metadata()
                .ok()
                .map(|m| m.modified().ok())
                .unwrap_or_default()
                .unwrap_or(SystemTime::now())
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            (name, modified)
        })
        .filter(|(_, modified)| {
            let current_time = *modified;
            println!(
                "[Running DenoTool] Modified: {}, Start Time: {}",
                current_time, start_time
            );
            println!("[Running DenoTool] Diff: {}", current_time >= start_time);
            current_time >= start_time
        })
        .collect()
}

pub fn update_result_with_modified_files(
    result: RunResult,
    start_time: u64,
    home_path: &PathBuf,
    logs_path: &PathBuf,
    node_name: &ShinkaiName,
    app_id: &str,
) -> Result<RunResult, ToolError> {
    if let serde_json::Value::Object(ref mut data) = result.clone().data {
        let modified_files = get_files_after(
            start_time,
            get_files_in_directories(vec![home_path, logs_path]).unwrap_or_default(),
        );
        data.insert(
            "__created_files__".to_string(),
            serde_json::Value::Array(
                modified_files
                    .into_iter()
                    .map(|(name, _)| {
                        serde_json::Value::String(convert_to_shinkai_file_protocol(&node_name, &name, &app_id))
                    })
                    .collect(),
            ),
        );
        Ok(RunResult {
            data: serde_json::Value::Object(data.clone()),
        })
    } else {
        println!("[Running DenoTool] Result is not an object, skipping modified files");
        return Err(ToolError::ExecutionError(
            "Result is not an object, skipping modified files".to_string(),
        ));
    }
}
