use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::CallbackAction;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{cron_tasks::cron_manager::CronManager, managers::sheet_manager::SheetManager};

use super::job_manager::JobManager;

/// The `JobCallbackManager` is responsible for handling incoming job requests
/// and delegating them to the appropriate manager (JobManager, SheetManager, or CronManager).
///
/// # Fields
/// - `job_manager`: An `Arc<Mutex<JobManager>>` for handling job-related requests.
/// - `sheet_manager`: An `Arc<Mutex<SheetManager>>` for handling sheet-related requests.
/// - `cron_manager`: An `Arc<Mutex<CronManager>>` for handling cron-related requests.
///
/// # Methods
/// - `new`: Creates a new instance of `JobCallbackManager` with the provided managers.
/// - `handle_request`: Takes a `JobRequest` and forwards it to the appropriate manager based on the `manager_type`.
pub struct JobCallbackManager {
    job_manager: Option<Arc<Mutex<JobManager>>>,
    sheet_manager: Option<Arc<Mutex<SheetManager>>>,
    cron_manager: Option<Arc<Mutex<CronManager>>>,
}

// TODO: allow for chaining of multiple jobs some of the jobs may give a result that's used by another job A -> B -> C
impl Default for JobCallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobCallbackManager {
    pub fn new() -> Self {
        JobCallbackManager {
            job_manager: None,
            sheet_manager: None,
            cron_manager: None,
        }
    }

    pub fn update_job_manager(&mut self, job_manager: Arc<Mutex<JobManager>>) {
        self.job_manager = Some(job_manager);
    }

    pub fn update_sheet_manager(&mut self, sheet_manager: Arc<Mutex<SheetManager>>) {
        self.sheet_manager = Some(sheet_manager);
    }

    pub fn update_cron_manager(&mut self, cron_manager: Arc<Mutex<CronManager>>) {
        self.cron_manager = Some(cron_manager);
    }

    // pub async fn handle_request(&self, action: CallbackAction) {
    //     match action {
    //         // CallbackAction::Job(job_message) => {
    //         //     let mut manager = self.job_manager.lock().await;
    //         //     // manager.handle_job(job_message);
    //         // }
    //         CallbackAction::Sheet(sheet_action) => {
    //             let mut manager = self.sheet_manager.lock().await;
    //             // manager.handle_sheet(sheet_action);
    //         }
    //         // Note: add later
    //         // CallbackAction::Cron(cron_action) => {
    //         //     let mut manager = self.cron_manager.lock().await;
    //         //     // manager.handle_cron(cron_action);
    //         // }
    //     }

    //     // // Handle nested callbacks
    //     // if let Some(callback) = action.get_callback() {
    //     //     self.handle_request(*callback);
    //     // }
    // }
}
