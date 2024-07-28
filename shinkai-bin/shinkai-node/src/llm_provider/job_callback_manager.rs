use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

use crate::{cron_tasks::cron_manager::CronManager, managers::sheet_manager::SheetManager};

use super::job_manager::JobManager;

// Define the enum for manager types
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CallbackManagerType {
    Job,
    Sheet,
    Cron,
}

// Define the serialized struct for the request
#[derive(Serialize, Deserialize)]
pub struct CallbackJobRequest {
    manager_type: CallbackManagerType,
    payload: String,
}

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
    job_manager: Arc<Mutex<JobManager>>,
    sheet_manager: Arc<Mutex<SheetManager>>,
    cron_manager: Arc<Mutex<CronManager>>,
}

// TODO: allow for chaining of multiple jobs some of the jobs may give a result that's used by another job A -> B -> C

impl JobCallbackManager {
    pub fn new(
        job_manager: Arc<Mutex<JobManager>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        cron_manager: Arc<Mutex<CronManager>>,
    ) -> Self {
        JobCallbackManager {
            job_manager,
            sheet_manager,
            cron_manager,
        }
    }

    fn handle_request(&self, request: CallbackJobRequest) {
        match request.manager_type {
            CallbackManagerType::Job => {
                let mut manager = self.job_manager.lock().unwrap();
                // manager.handle_job(request.payload);
            }
            CallbackManagerType::Sheet => {
                let mut manager = self.sheet_manager.lock().unwrap();
                // manager.handle_sheet(request.payload);
            }
            CallbackManagerType::Cron => {
                let mut manager = self.cron_manager.lock().unwrap();
                // manager.handle_cron(request.payload);
            }
        }
    }
}

// // Assume these are defined elsewhere
// struct JobManager;
// impl JobManager {
//     fn handle_job(&mut self, payload: String) {
//         // Handle job
//     }
// }

// struct SheetManager;
// impl SheetManager {
//     fn handle_sheet(&mut self, payload: String) {
//         // Handle sheet
//     }
// }

// struct CronManager;
// impl CronManager {
//     fn handle_cron(&mut self, payload: String) {
//         // Handle cron
//     }
// }