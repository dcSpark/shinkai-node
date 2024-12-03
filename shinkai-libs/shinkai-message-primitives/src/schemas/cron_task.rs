use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronTask {
    pub task_id: String,
    pub cron: String,
    pub prompt: String,
    pub subprompt: String,
    pub url: String,
    pub crawl_links: bool,
    pub created_at: String,
    pub llm_provider_id: String,
}

impl PartialOrd for CronTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CronTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.task_id.cmp(&other.task_id)
    }
}
