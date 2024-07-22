use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::fmt::Debug;
use crate::sheet::CellId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

pub trait SheetJob: Send + Sync {
    fn id(&self) -> &str;
    fn cell_id(&self) -> &CellId;
    fn prompt(&self) -> &str;
    fn dependencies(&self) -> &[CellId];
    fn status(&self) -> JobStatus;
    fn created_at(&self) -> DateTime<Utc>;
    fn updated_at(&self) -> DateTime<Utc>;
    fn result(&self) -> Option<&str>;
    fn set_status(&mut self, status: JobStatus);
    fn set_result(&mut self, result: String);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockupSheetJob {
    id: String,
    cell_id: CellId,
    prompt: String,
    dependencies: Vec<CellId>,
    status: JobStatus,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    result: Option<String>,
}


impl SheetJob for MockupSheetJob {
    fn id(&self) -> &str { &self.id }
    fn cell_id(&self) -> &CellId { &self.cell_id }
    fn prompt(&self) -> &str { &self.prompt }
    fn dependencies(&self) -> &[CellId] { &self.dependencies }
    fn status(&self) -> JobStatus { self.status.clone() }
    fn created_at(&self) -> DateTime<Utc> { self.created_at }
    fn updated_at(&self) -> DateTime<Utc> { self.updated_at }
    fn result(&self) -> Option<&str> { self.result.as_deref() }
    fn set_status(&mut self, status: JobStatus) { 
        self.status = status;
        self.updated_at = Utc::now();
    }
    fn set_result(&mut self, result: String) { 
        self.result = Some(result);
        self.updated_at = Utc::now();
    }
}

impl MockupSheetJob {
    pub fn new(id: String, cell_id: CellId, prompt: String, dependencies: Vec<CellId>) -> Self {
        let now = Utc::now();
        Self {
            id,
            cell_id,
            prompt,
            dependencies,
            status: JobStatus::Pending,
            created_at: now,
            updated_at: now,
            result: None,
        }
    }
}