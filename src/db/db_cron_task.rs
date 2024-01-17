use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic, db_profile_bound::ProfileBoundWriteBatch};
use chrono::Utc;
use rocksdb::{IteratorMode, Options};
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::{schemas::shinkai_name::ShinkaiName, shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogOption, ShinkaiLogLevel}};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronTask {
    pub task_id: String,
    pub cron: String,
    pub prompt: String,
    pub subprompt: String,
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
        profile: ShinkaiName,
        task_id: String,
        cron: String,
        prompt: String,
        subprompt: String,
        url: String,
        crawl_links: bool,
        agent_id: String,
    ) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;

        let cf_name_schedule = format!("{}_cron_task_schedule", profile_name);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile_name);
        let cf_name_subprompt = format!("{}_cron_task_subprompt", profile_name);
        let cf_name_url = format!("{}_cron_task_url", profile_name);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile_name);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile_name);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile_name);

        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        if self.db.cf_handle(&cf_name_schedule).is_none() {
            self.db.create_cf(&cf_name_schedule, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_prompt).is_none() {
            self.db.create_cf(&cf_name_prompt, &cf_opts)?;
        }

        if self.db.cf_handle(&cf_name_subprompt).is_none() {
            self.db.create_cf(&cf_name_subprompt, &cf_opts)?;
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

        let cf_subprompt = self
            .db
            .cf_handle(&cf_name_subprompt)
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

        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let mut pb_batch = ProfileBoundWriteBatch::new(&profile)?;

        pb_batch.pb_put_cf(cf_schedule, &task_id, &cron);
        pb_batch.pb_put_cf(cf_prompt, &task_id, &prompt);
        pb_batch.pb_put_cf(cf_subprompt, &task_id, &subprompt);
        pb_batch.pb_put_cf(cf_url, &task_id, &url);
        pb_batch.pb_put_cf(cf_crawl_links, &task_id, &crawl_links.to_string());

        let created_at = Utc::now().to_rfc3339();
        pb_batch.pb_put_cf(cf_created_at, &task_id, &created_at);
        pb_batch.pb_put_cf(cf_agent_id, &task_id, &agent_id);
        pb_batch.pb_put_cf(cf_cron_queues, &task_id, &profile_name);

        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn remove_cron_task(&mut self, profile: ShinkaiName, task_id: String) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;

        let cf_name_schedule = format!("{}_cron_task_schedule", profile_name);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile_name);
        let cf_name_subprompt = format!("{}_cron_task_subprompt", profile_name);
        let cf_name_url = format!("{}_cron_task_url", profile_name);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile_name);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile_name);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile_name);
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for schedule: {}",
                task_id
            )))?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for prompt: {}",
                task_id
            )))?;

        let cf_subprompt = self
            .db
            .cf_handle(&cf_name_subprompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for prompt: {}",
                task_id
            )))?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found for url: {}",
                task_id
            )))?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron task not found: {}",
                task_id
            )))?;

        let mut pb_batch = ProfileBoundWriteBatch::new(&profile)?;
        pb_batch.pb_delete_cf(cf_schedule, &task_id);
        pb_batch.pb_delete_cf(cf_prompt, &task_id);
        pb_batch.pb_delete_cf(cf_subprompt, &task_id);
        pb_batch.pb_delete_cf(cf_url, &task_id);
        pb_batch.pb_delete_cf(cf_crawl_links, &task_id);
        pb_batch.pb_delete_cf(cf_created_at, &task_id);
        pb_batch.pb_delete_cf(cf_agent_id, &task_id);
        pb_batch.pb_delete_cf(cf_cron_queues, &task_id);

        self.write_pb(pb_batch)?;
        Ok(())
    }

    pub fn get_all_cron_tasks_from_all_profiles(
        &self,
        node_name: ShinkaiName,
    ) -> Result<HashMap<String, Vec<(String, CronTask)>>, ShinkaiDBError> {
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let mut all_profiles = HashSet::new();
        for result in self.db.iterator_cf(cf_cron_queues, IteratorMode::Start) {
            match result {
                Ok((_, value)) => {
                    let profile = String::from_utf8(value.to_vec()).unwrap();
                    shinkai_log(
                        ShinkaiLogOption::CronExecution,
                        ShinkaiLogLevel::Debug,
                        format!("get_all_cron_tasks_from_all_profiles profile: {:?}", profile).as_str(),
                    );
                    all_profiles.insert(profile);
                }
                Err(e) => return Err(e.into()),
            }
        }

        let mut all_tasks = HashMap::new();
        for profile in all_profiles.clone() {
            let shinkai_profile = ShinkaiName::from_node_and_profile(node_name.get_node_name(), profile.clone())?;
            let tasks = self.get_all_cron_tasks_for_profile(shinkai_profile)?;
            for (task_id, task) in tasks {
                all_tasks
                    .entry(profile.clone())
                    .or_insert_with(Vec::new)
                    .push((task_id, task));
            }
        }

        Ok(all_tasks)
    }

    pub fn get_all_cron_tasks_for_profile(
        &self,
        profile: ShinkaiName,
    ) -> Result<HashMap<String, CronTask>, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;
        let cf_name_schedule = format!("{}_cron_task_schedule", profile_name);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile_name);
        let cf_name_subprompt = format!("{}_cron_task_subprompt", profile_name);
        let cf_name_url = format!("{}_cron_task_url", profile_name);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile_name);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile_name);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile_name);

        let cf_schedule = self
            .db
            .cf_handle(&cf_name_schedule)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (name_schedule) not found for profile: {}",
                profile_name
            )))?;

        let cf_prompt = self
            .db
            .cf_handle(&cf_name_prompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (name_prompt) not found for profile: {}",
                profile_name
            )))?;

        let cf_subprompt = self
            .db
            .cf_handle(&cf_name_subprompt)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (name_prompt) not found for profile: {}",
                profile_name
            )))?;

        let cf_url = self
            .db
            .cf_handle(&cf_name_url)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (name_url) not found for profile: {}",
                profile_name
            )))?;

        let cf_crawl_links = self
            .db
            .cf_handle(&cf_name_crawl_links)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (crawl_links) not found for profile: {}",
                profile_name
            )))?;

        let cf_created_at = self
            .db
            .cf_handle(&cf_name_created_at)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (created_at) not found for profile: {}",
                profile_name
            )))?;

        let cf_agent_id = self
            .db
            .cf_handle(&cf_name_agent_id)
            .ok_or(ShinkaiDBError::CronTaskNotFound(format!(
                "Cron tasks (agent_id) not found for profile: {}",
                profile_name
            )))?;

        let mut tasks = HashMap::new();
        for result in self.db.iterator_cf(cf_schedule, IteratorMode::Start) {
            match result {
                Ok((key, value)) => {
                    let task_id = String::from_utf8(key.to_vec()).unwrap();
                    let cron = String::from_utf8(value.to_vec()).unwrap();
                    let prompt = String::from_utf8(self.db.get_cf(cf_prompt, &task_id)?.unwrap_or_default()).unwrap();
                    let subprompt =
                        String::from_utf8(self.db.get_cf(cf_subprompt, &task_id)?.unwrap_or_default()).unwrap();
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
                            subprompt,
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

    pub fn get_cron_task(&self, profile: ShinkaiName, task_id: String) -> Result<CronTask, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;

        let cf_name_schedule = format!("{}_cron_task_schedule", profile_name);
        let cf_name_prompt = format!("{}_cron_task_prompt", profile_name);
        let cf_name_subprompt = format!("{}_cron_task_subprompt", profile_name);
        let cf_name_url = format!("{}_cron_task_url", profile_name);
        let cf_name_crawl_links = format!("{}_cron_task_crawl_links", profile_name);
        let cf_name_created_at = format!("{}_cron_task_created_at", profile_name);
        let cf_name_agent_id = format!("{}_cron_task_agent_id", profile_name);

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

        let cf_subprompt = self
            .db
            .cf_handle(&cf_name_subprompt)
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
        let subprompt = String::from_utf8(self.db.get_cf(cf_subprompt, &task_id)?.unwrap_or_default()).unwrap();
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
            subprompt,
            url,
            crawl_links,
            created_at,
            agent_id,
        })
    }
}
