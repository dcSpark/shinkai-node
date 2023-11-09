use std::{cmp::Ordering, collections::HashMap};

use super::{db_errors::ShinkaiDBError, ShinkaiDB};
use chrono::Utc;
use rocksdb::{IteratorMode, Options};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronTask {
    pub task_id: String,
    pub cron: String,
    pub prompt: String,
    pub url: String,
    pub crawl_links: bool,
    pub created_at: String,
    pub agent_id: String,
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
        crawl_links: bool,
        agent_id: String,
    ) -> Result<(), ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);
        let cf_name_crawl_links = format!("{}_cron_crawl_links", profile);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile);

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

        if self.db.cf_handle(&cf_name_crawl_links).is_none() {
            self.db.create_cf(&cf_name_crawl_links, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_created_at).is_none() {
            self.db.create_cf(&cf_name_created_at, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_agent_id).is_none() {
            self.db.create_cf(&cf_name_agent_id, &cf_opts)?;
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

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_schedule, &task_id, &cron);
        batch.put_cf(cf_prompt, &task_id, &prompt);
        batch.put_cf(cf_url, &task_id, &url);
        batch.put_cf(cf_crawl_links, &task_id, &crawl_links.to_string());

        let created_at = Utc::now().to_rfc3339();
        batch.put_cf(cf_created_at, &task_id, &created_at);
        batch.put_cf(cf_agent_id, &task_id, &agent_id);

        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_cron_task(&mut self, profile: String, task_id: String) -> Result<(), ShinkaiDBError> {
        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile);

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

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_crawl_links, &task_id)?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_created_at, &task_id)?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;
        self.db.delete_cf(cf_agent_id, &task_id)?;

        Ok(())
    }

    pub fn get_all_cron_tasks(&self, profile: String) -> Result<HashMap<String, CronTask>, ShinkaiDBError> {
        eprintln!("get_all_cron_tasks for profile: {}", profile);

        let cf_name_schedule = format!("{}_cron_task_schedule", profile);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile);
        let cf_name_url = format!("{}_cron_task_url", profile);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile);

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

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks not found for profile: {}",
                profile
            )))?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks not found for profile: {}",
                profile
            )))?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
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
                    let crawl_links = String::from_utf8(self.db.get_cf(cf_crawl_links, &task_id)?.unwrap_or_default())
                        .unwrap()
                        .parse::<bool>()
                        .unwrap();
                    let created_at =
                        String::from_utf8(self.db.get_cf(cf_created_at, &task_id)?.unwrap_or_default()).unwrap();
                    let agent_id =
                        String::from_utf8(self.db.get_cf(cf_agent_id, &task_id)?.unwrap_or_default()).unwrap();

                    tasks.insert(
                        task_id.clone(),
                        CronTask {
                            task_id,
                            cron,
                            prompt,
                            url,
                            crawl_links,
                            created_at,
                            agent_id,
                        },
                    );
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
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile);

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

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let cron = String::from_utf8(self.db.get_cf(cf_schedule, &task_id)?.unwrap_or_default()).unwrap();
        let prompt = String::from_utf8(self.db.get_cf(cf_prompt, &task_id)?.unwrap_or_default()).unwrap();
        let url = String::from_utf8(self.db.get_cf(cf_url, &task_id)?.unwrap_or_default()).unwrap();
        let crawl_links = String::from_utf8(self.db.get_cf(cf_crawl_links, &task_id)?.unwrap_or_default())
            .unwrap()
            .parse::<bool>()
            .unwrap();
        let created_at = String::from_utf8(self.db.get_cf(cf_created_at, &task_id)?.unwrap_or_default()).unwrap();
        let agent_id = String::from_utf8(self.db.get_cf(cf_agent_id, &task_id)?.unwrap_or_default()).unwrap();

        Ok(CronTask {
            task_id,
            cron,
            prompt,
            url,
            crawl_links,
            created_at,
            agent_id
        })
    }
}
