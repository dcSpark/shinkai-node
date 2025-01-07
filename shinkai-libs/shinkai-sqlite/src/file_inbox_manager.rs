use rusqlite::params;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    fn sanitize_folder_name(inbox_name: &str) -> String {
        let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        let sanitized_name: String = inbox_name
            .chars()
            .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
            .collect();

        // Trim any trailing whitespace
        sanitized_name.trim_end().to_string()
    }

    pub fn get_and_create_job_folder(&self, job_id: &str) -> Result<ShinkaiPath, SqliteManagerError> {
        // Get the job folder name
        let folder_path = self.get_job_folder_name(job_id)?;

        // Create the folder if it doesn't exist
        if !folder_path.exists() {
            std::fs::create_dir_all(&folder_path.path).map_err(|_| SqliteManagerError::FailedFetchingValue)?;
        }

        Ok(folder_path)
    }

    pub fn get_job_folder_name(&self, job_id: &str) -> Result<ShinkaiPath, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT conversation_inbox_name, datetime_created FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;
        let conversation_inbox_name: String = row.get(0)?;
        let datetime_created: String = row.get(1)?;

        // Fetch the smart inbox name using the conversation_inbox_name
        let smart_inbox_name = self.get_smart_inbox_name(&conversation_inbox_name)?;

        // Format the datetime_created to a more readable format
        let date = chrono::NaiveDateTime::parse_from_str(&datetime_created, "%Y-%m-%dT%H:%M:%S%.fZ")?;
        let formatted_date = date.format("%b %d").to_string();

        // Extract the last 4 characters of the job_id
        let job_id_suffix = &job_id[job_id.len() - 4..];

        // Create the folder name with the job_id suffix
        let folder_name = format!("{} - ({}) {}", formatted_date, job_id_suffix, smart_inbox_name);
        
        // Use the sanitize_folder_name function to ensure compatibility
        let valid_folder_name = Self::sanitize_folder_name(&folder_name);

        // Truncate if the name is too long
        let max_length = 30; // Max length
        let final_folder_name = if valid_folder_name.len() > max_length {
            valid_folder_name[..max_length].to_string()
        } else {
            valid_folder_name
        };

        // Trim any trailing whitespace from the final folder name
        let trimmed_final_folder_name = final_folder_name.trim_end().to_string();

        // Include the Chat Files folder in the path
        let full_path = format!("Chat Files/{}", trimmed_final_folder_name);

        Ok(ShinkaiPath::from_string(full_path))
    }
}
