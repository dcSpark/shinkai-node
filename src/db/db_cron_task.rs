use std::{collections::HashMap, cmp::Ordering};

use super::{db_errors::ShinkaiDBError, ShinkaiDB};
use rocksdb::{IteratorMode, Options};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronTask {
    pub task_id: String,
    pub cron: String,
    pub prompt: String,
    pub url: String,
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

impl ShinkaiDB {
    pub fn add_cron_task(
        &mut self,
        profile: String,
        task_id: String,
        cron: String,
        prompt: String,
        url: String,
    ) -> Result<(), ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        if self.db.cf_handle(&cf_name_schedule).is_none() {
            self.db.create_cf(&cf_name_schedule, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_prompt).is_none() {
            self.db.create_cf(&cf_name_prompt, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_url).is_none() {
            self.db.create_cf(&cf_name_url, &cf_opts)?;
        }

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_schedule, &task_id, &cron);
        batch.put_cf(cf_prompt, &task_id, &prompt);
        batch.put_cf(cf_url, &task_id, &url);

        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_cron_task(&mut self, profile: String, task_id: String) -> Result<(), ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for schedule: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_schedule, &task_id)?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for prompt: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_prompt, &task_id)?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_url, &task_id)?;

        Ok(())
    }

    pub fn get_all_cron_tasks(&self, profile: String) -> Result<HashMap<String, CronTask>, ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks not found for profile: {}",
                profile
            )))?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks not found for profile: {}",
                profile
            )))?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks not found for profile: {}",
                profile
            )))?;

        let mut tasks = HashMap::new();
        for result in self.db.iterator_cf(cf_schedule, IteratorMode::Start) {
            match result {
                Ok((key, value)) => {
                    let task_id = String::from_utf8(key.to_vec()).unwrap();
                    let cron = String::from_utf8(value.to_vec()).unwrap();
                    let prompt = String::from_utf8(self.db.get_cf(cf_prompt, &task_id)?.unwrap_or_default()).unwrap();
                    let url = String::from_utf8(self.db.get_cf(cf_url, &task_id)?.unwrap_or_default()).unwrap();
                    tasks.insert(task_id.clone(), CronTask { task_id, cron, prompt, url });
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(tasks)
    }

    pub fn get_cron_task(&self, profile: String, task_id: String) -> Result<CronTask, ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cron = String::from_utf8(self.db.get_cf(cf_schedule, &task_id)?.unwrap_or_default()).unwrap();
        let prompt = String::from_utf8(self.db.get_cf(cf_prompt, &task_id)?.unwrap_or_default()).unwrap();
        let url = String::from_utf8(self.db.get_cf(cf_url, &task_id)?.unwrap_or_default()).unwrap();

        Ok(CronTask { task_id, cron, prompt, url })
    }
}
