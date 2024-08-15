use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::llm_provider::job_manager::JobManager;
use crate::network::ws_manager::{WSMessageType, WSUpdateHandler};
use async_channel::{Receiver, Sender};
use shinkai_message_primitives::schemas::sheet::{
    APIColumnDefinition, ColumnDefinition, ColumnUuid, RowUuid, WorkflowSheetJobData,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    CallbackAction, JobCreationInfo, JobMessage, SheetJobAction, SheetManagerAction, WSTopic,
};
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_sheet::cell_name_converter::CellNameConverter;
use shinkai_sheet::sheet::{Sheet, SheetUpdate};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug)]
pub struct SheetManagerError(String);

impl std::fmt::Display for SheetManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SheetManagerError {}

pub struct SheetManager {
    pub sheets: HashMap<String, (Sheet, Sender<SheetUpdate>)>,
    pub db: Weak<ShinkaiDB>,
    pub user_profile: ShinkaiName,
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    pub update_handles: Vec<JoinHandle<()>>,
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    pub receivers: HashMap<String, Receiver<SheetUpdate>>, // to avoid premature drops
}

// TODO: add blacklist property so we can to stop chains (when user cancels a job, we should stop the chain)

impl SheetManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        node_name: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<Self, SheetManagerError> {
        let user_profile = ShinkaiName::from_node_and_profile_names(node_name.node_name, "main".to_string())
            .map_err(|e| SheetManagerError(e.to_string()))?;

        let db_strong = db
            .upgrade()
            .ok_or_else(|| SheetManagerError("Couldn't convert to strong db".to_string()))?;

        let sheets_vec = db_strong
            .list_all_sheets_for_user(&user_profile)
            .map_err(|e| SheetManagerError(e.to_string()))?;

        let mut update_handles = Vec::new();
        let mut receivers = HashMap::new();

        let sheets = sheets_vec
            .into_iter()
            .map(|mut sheet| {
                let (sender, receiver) = async_channel::unbounded();
                sheet.set_update_sender(sender.clone());
                // Start a task to handle updates
                let handle = tokio::spawn(Self::handle_updates(receiver.clone(), ws_manager.clone()));
                update_handles.push(handle);
                receivers.insert(sheet.uuid.clone(), receiver);
                (sheet.uuid.clone(), (sheet, sender))
            })
            .collect();

        Ok(Self {
            sheets,
            db,
            job_manager: None,
            user_profile,
            update_handles,
            ws_manager,
            receivers,
        })
    }

    pub fn set_job_manager(&mut self, job_manager: Arc<Mutex<JobManager>>) {
        self.job_manager = Some(job_manager);
    }

    pub fn create_empty_sheet(&mut self) -> Result<String, ShinkaiDBError> {
        let sheet = Sheet::new();
        let sheet_id = sheet.uuid.clone();
        let (sender, receiver) = async_channel::unbounded();
        let mut sheet_clone = sheet.clone();
        sheet_clone.set_update_sender(sender.clone());

        self.sheets.insert(sheet_id.clone(), (sheet_clone, sender));
        self.receivers.insert(sheet_id.clone(), receiver.clone());

        // Start a task to handle updates
        let handle = tokio::spawn(Self::handle_updates(receiver, self.ws_manager.clone()));
        self.update_handles.push(handle);

        // Add the sheet to the database
        let db_strong = self
            .db
            .upgrade()
            .ok_or(ShinkaiDBError::SomeError("Couldn't convert to strong db".to_string()))?;
        db_strong.save_sheet(sheet, self.user_profile.clone())?;

        Ok(sheet_id)
    }

    pub fn get_sheet(&self, sheet_id: &str) -> Result<Sheet, String> {
        self.sheets
            .get(sheet_id)
            .map(|(sheet, _)| sheet)
            .ok_or_else(|| "Sheet ID not found".to_string())
            .cloned()
    }

    pub fn add_sheet(&mut self, sheet: Sheet) -> Result<String, ShinkaiDBError> {
        let (sender, receiver) = async_channel::unbounded();
        let sheet_id = sheet.uuid.clone();
        let mut sheet_clone = sheet.clone();
        sheet_clone.set_update_sender(sender.clone());

        self.sheets.insert(sheet_id.clone(), (sheet_clone, sender));
        self.receivers.insert(sheet_id.clone(), receiver.clone());

        // Start a task to handle updates
        let handle = tokio::spawn(Self::handle_updates(receiver, self.ws_manager.clone()));
        self.update_handles.push(handle);

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

    pub async fn update_sheet_name(&mut self, sheet_id: &str, new_name: String) -> Result<(), String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;
        sheet.sheet_name = Some(new_name.clone());

        // Update the sheet in the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn duplicate_sheet(&mut self, sheet_id: &str, new_name: String) -> Result<String, String> {
        // Get the original sheet
        let (original_sheet, _) = self.sheets.get(sheet_id).ok_or("Sheet ID not found")?;

        // Clone the original sheet and assign a new UUID
        let mut new_sheet = original_sheet.clone();
        new_sheet.uuid = Uuid::new_v4().to_string();
        new_sheet.sheet_name = Some(new_name.clone());

        // Create a new sender and receiver for the new sheet
        let (sender, receiver) = async_channel::unbounded();
        new_sheet.set_update_sender(sender.clone());

        // Insert the new sheet into the HashMap
        self.sheets.insert(new_sheet.uuid.clone(), (new_sheet.clone(), sender));

        // Add the new sheet to the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(new_sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

        // Start a task to handle updates for the new sheet
        let handle = tokio::spawn(Self::handle_updates(receiver, self.ws_manager.clone()));
        self.update_handles.push(handle);

        Ok(new_sheet.uuid)
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
            let agent_id = job_data.llm_provider_name.clone();
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
                    row: current_job_data.row.clone(),
                    col: current_job_data.col.clone(),
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

    pub async fn from_api_column_to_new_column(
        &self,
        sheet_id: &str,
        column: APIColumnDefinition,
    ) -> Result<ColumnDefinition, String> {
        let sheet = self.sheets.get(sheet_id).ok_or("Sheet ID not found")?.0.clone();

        let id = column.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let name = column.name.unwrap_or_else(|| {
            let col_name = CellNameConverter::column_index_to_name(sheet.columns.len());
            format!("Column {}", col_name)
        });

        Ok(ColumnDefinition {
            id,
            name,
            behavior: column.behavior,
        })
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

    pub async fn remove_column(&mut self, sheet_id: &str, column_id: ColumnUuid) -> Result<(), String> {
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

    pub async fn add_row(&mut self, sheet_id: &str, _position: Option<usize>) -> Result<RowUuid, String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;
        let row_id = Uuid::new_v4().to_string();
        let jobs = sheet.add_row(row_id.clone()).await.map_err(|e| e.to_string())?;

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

        Ok(row_id)
    }

    pub async fn remove_rows(&mut self, sheet_id: &str, row_indices: Vec<RowUuid>) -> Result<(), String> {
        let (sheet, _) = self.sheets.get_mut(sheet_id).ok_or("Sheet ID not found")?;

        for row_index in row_indices {
            sheet.remove_row(row_index).await.map_err(|e| e.to_string())?;
        }

        // Update the sheet in the database
        let db_strong = self.db.upgrade().ok_or("Couldn't convert to strong db".to_string())?;
        db_strong
            .save_sheet(sheet.clone(), self.user_profile.clone())
            .map_err(|e| e.to_string())?;

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
        row: RowUuid,
        col: ColumnUuid,
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

    pub fn get_cell_value(&self, sheet_id: &str, row: RowUuid, col: ColumnUuid) -> Result<Option<String>, String> {
        let sheet = self.sheets.get(sheet_id).ok_or("Sheet ID not found")?.0.clone();
        Ok(sheet.get_cell_value(row, col))
    }

    pub fn set_update_sender(&mut self, id: &str, sender: Sender<SheetUpdate>) -> Result<(), String> {
        if let Some((sheet, _)) = self.sheets.get_mut(id) {
            sheet.set_update_sender(sender);
            Ok(())
        } else {
            Err("Sheet ID not found".to_string())
        }
    }

    async fn handle_updates(
        receiver: Receiver<SheetUpdate>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) {
        while let Ok(update) = receiver.recv().await {
            // Handle the update (e.g., log it, process it, etc.)
            // TODO: check from which sheet the update came from
            println!("Received update: {:?}", update);

            if let Some(ws_manager) = &ws_manager {
                let ws_manager = ws_manager.lock().await;

                match update {
                    SheetUpdate::CellUpdated(cell_update_info) => {
                        let topic = WSTopic::Sheet;
                        let subtopic = cell_update_info.sheet_id.clone();
                        let update = serde_json::to_string(&cell_update_info).unwrap();
                        let metadata = WSMessageType::Sheet(cell_update_info);
                        ws_manager.queue_message(topic, subtopic, update, metadata, false).await;
                    }
                }
            }
        }
    }
}
