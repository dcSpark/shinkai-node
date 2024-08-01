use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::llm_provider::job_manager::JobManager;
use async_channel::{Receiver, Sender};
use shinkai_message_primitives::schemas::sheet::{ColumnDefinition, WorkflowSheetJobData};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    CallbackAction, JobCreationInfo, JobMessage, SheetJobAction, SheetManagerAction,
};
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_sheet::sheet::{Sheet, SheetUpdate};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct SheetManagerError(String);

impl std::fmt::Display for SheetManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SheetManagerError {}

#[derive(Clone)]
pub struct SheetManager {
    pub sheets: HashMap<String, (Sheet, Sender<SheetUpdate>)>,
    pub db: Weak<ShinkaiDB>,
    pub user_profile: ShinkaiName,
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
}

// TODO: add blacklist property so we can to stop chains (when user cancels a job, we should stop the chain)

impl SheetManager {
    pub async fn new(db: Weak<ShinkaiDB>, node_name: ShinkaiName) -> Result<Self, SheetManagerError> {
        // Only works for main right now
        let user_profile = ShinkaiName::from_node_and_profile_names(node_name.node_name, "main".to_string())
            .map_err(|e| SheetManagerError(e.to_string()))?;

        let db_strong = db
            .upgrade()
            .ok_or_else(|| SheetManagerError("Couldn't convert to strong db".to_string()))?;

        let sheets_vec = db_strong
            .list_all_sheets_for_user(&user_profile)
            .map_err(|e| SheetManagerError(e.to_string()))?;

        let sheets = sheets_vec
            .into_iter()
            .map(|mut sheet| {
                let (sender, receiver) = async_channel::unbounded();
                sheet.set_update_sender(sender.clone());
                // Start a task to handle updates
                tokio::spawn(Self::handle_updates(receiver));
                (sheet.uuid.clone(), (sheet, sender))
            })
            .collect();

        Ok(Self {
            sheets,
            db,
            job_manager: None,
            user_profile,
        })
    }

    pub fn set_job_manager(&mut self, job_manager: Arc<Mutex<JobManager>>) {
        self.job_manager = Some(job_manager);
    }

    pub fn create_empty_sheet(&mut self) -> Result<(), ShinkaiDBError> {
        let sheet = Sheet::new();
        let sheet_id = sheet.uuid.clone();
        let (sender, _receiver) = async_channel::unbounded();
        let mut sheet_clone = sheet.clone();
        sheet_clone.set_update_sender(sender.clone());

        self.sheets.insert(sheet_id.clone(), (sheet_clone, sender));

        // Add the sheet to the database
        let db_strong = self
            .db
            .upgrade()
            .ok_or(ShinkaiDBError::SomeError("Couldn't convert to strong db".to_string()))?;
        db_strong.save_sheet(sheet, self.user_profile.clone())?;

        Ok(())
    }

    pub fn get_sheet(&self, sheet_id: &str) -> Result<&Sheet, String> {
        self.sheets
            .get(sheet_id)
            .map(|(sheet, _)| sheet)
            .ok_or_else(|| "Sheet ID not found".to_string())
    }

    pub fn add_sheet(&mut self, sheet: Sheet) -> Result<String, ShinkaiDBError> {
        let (sender, _receiver) = async_channel::unbounded();
        let sheet_id = sheet.uuid.clone();
        let mut sheet_clone = sheet.clone();
        sheet_clone.set_update_sender(sender.clone());

        self.sheets.insert(sheet_id.clone(), (sheet_clone, sender));

        // Add the sheet to the database
        let db_strong = self
            .db
            .upgrade()
            .ok_or(ShinkaiDBError::SomeError("Couldn't convert to strong db".to_string()))?;
        db_strong.save_sheet(sheet, self.user_profile.clone())?;

        Ok(sheet_id)
    }

    pub fn remove_sheet(&mut self, sheet_id: &str) -> Result<(), ShinkaiDBError> {
        // Remove the sheet from the HashMap
        if self.sheets.remove(sheet_id).is_none() {
            return Err(ShinkaiDBError::SomeError("Sheet ID not found".to_string()));
        }

        // Remove the sheet from the database
        let db_strong = self
            .db
            .upgrade()
            .ok_or(ShinkaiDBError::SomeError("Couldn't convert to strong db".to_string()))?;
        db_strong.remove_sheet(sheet_id, &self.user_profile)?;

        Ok(())
    }

    async fn create_and_chain_job_messages(
        jobs: Vec<WorkflowSheetJobData>,
        job_manager: &Arc<Mutex<JobManager>>,
        user_profile: &ShinkaiName,
    ) -> Result<(), String> {
        let mut job_messages: Vec<(JobMessage, WorkflowSheetJobData)> = Vec::new();

        for job_data in jobs {
            let job_creation_info = JobCreationInfo {
                scope: JobScope::new_default(),
                is_hidden: Some(true),
            };

            let mut job_manager = job_manager.lock().await;
            let agent_name =
                ShinkaiName::from_node_and_profile_names(user_profile.node_name.clone(), "main".to_string())?;
            let agent_id = agent_name.get_agent_name_string().ok_or("LLMProviderNotFound")?;
            let job_id = job_manager
                .process_job_creation(job_creation_info, user_profile, &agent_id)
                .await
                .map_err(|e| e.to_string())?;

            let job_message = JobMessage {
                job_id: job_id.clone(),
                content: "".to_string(), // it could be in the sheet_job_data (indirectly through reading the cell)
                files_inbox: "".to_string(), // it could be in the sheet_job_data (indirectly through reading the cell)
                parent: None,
                workflow_code: None, // it could be in the sheet_job_data
                workflow_name: None, // it could be in the sheet_job_data
                sheet_job_data: Some(serde_json::to_string(&job_data).unwrap()),
                callback: None,
            };

            job_messages.push((job_message, job_data));
        }

        // Chain the JobMessages with SheetManagerAction
        for i in (1..job_messages.len()).rev() {
            let (next_job_message, _next_job_data) = job_messages[i].clone();
            let (current_job_message, current_job_data) = &mut job_messages[i - 1];
            current_job_message.callback = Some(Box::new(CallbackAction::Sheet(SheetManagerAction {
                job_message_next: Some(next_job_message),
                sheet_action: SheetJobAction {
                    sheet_id: current_job_data.sheet_id.clone(),
                    row: current_job_data.row,
                    col: current_job_data.col,
                },
            })));
        }

        // Add the first JobMessage to the job queue
        if let Some((first_job_message, _)) = job_messages.first() {
            let mut job_manager = job_manager.lock().await;
            job_manager
                .add_job_message_to_job_queue(first_job_message, user_profile)
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub async fn set_column(&mut self, sheet_id: &str, column: ColumnDefinition) -> Result<(), String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;
        let jobs = sheet.set_column(column.clone()).await.map_err(|e| e.to_string())?;

        // Update the sheet in the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

        // Create and chain JobMessages, and add the first one to the job queue
        // Create and chain JobMessages, and add the first one to the job queue
        if let Some(job_manager) = &self.job_manager {
            Self::create_and_chain_job_messages(jobs, job_manager, &self.user_profile).await?;
        } else {
            return Err("JobManager not set".to_string());
        }

        Ok(())
    }

    pub async fn remove_column(&mut self, sheet_id: &str, column_id: usize) -> Result<(), String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;
        let jobs = sheet.remove_column(column_id).await.map_err(|e| e.to_string())?;

        // Update the sheet in the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

        // Create and chain JobMessages, and add the first one to the job queue
        if let Some(job_manager) = &self.job_manager {
            Self::create_and_chain_job_messages(jobs, job_manager, &self.user_profile).await?;
        } else {
            return Err("JobManager not set".to_string());
        }

        Ok(())
    }

    pub async fn get_user_sheets(&self) -> Result<Vec<Sheet>, String> {
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .list_all_sheets_for_user(&self.user_profile)
            .map_err(|e| e.to_string())
    }

    pub async fn set_cell_value(
        &mut self,
        sheet_id: &str,
        row: usize,
        col: usize,
        value: String,
    ) -> Result<(), String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;
        let jobs = sheet.set_cell_value(row, col, value).await?;

        // Update the sheet in the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

        // Create and chain JobMessages, and add the first one to the job queue
        if let Some(job_manager) = &self.job_manager {
            Self::create_and_chain_job_messages(jobs, job_manager, &self.user_profile).await?;
        } else {
            return Err("JobManager not set".to_string());
        }

        Ok(())
    }

    pub fn set_update_sender(&mut self, id: &str, sender: Sender<SheetUpdate>) -> Result<(), String> {
        if let Some((sheet, _)) = self.sheets.get_mut(id) {
            sheet.set_update_sender(sender);
            Ok(())
        } else {
            Err("Sheet ID not found".to_string())
        }
    }

    async fn handle_updates(receiver: Receiver<SheetUpdate>) {
        while let Ok(update) = receiver.recv().await {
            // Handle the update (e.g., log it, process it, etc.)
            // TODO: check from which sheet the update came from
            println!("Received update: {:?}", update);
        }
    }
}
