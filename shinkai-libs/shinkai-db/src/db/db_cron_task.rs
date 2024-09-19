use std::{cmp::Ordering, collections::HashMap};

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use chrono::Utc;

use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

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

impl ShinkaiDB {
    #[allow(clippy::too_many_arguments)]
    pub fn add_cron_task(
        &self,
        profile: ShinkaiName,
        task_id: String,
        cron: String,
        prompt: String,
        subprompt: String,
        url: String,
        crawl_links: bool,
        llm_provider_id: String,
    ) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;

        // Use Topic::CronQueues with standard prefixes for each task attribute
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let prefix = format!("{}_{}", profile_name, task_id);

        // Start a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Store each attribute of the cron task with a unique prefix
        batch.put_cf(cf_cron_queues, format!("{}_cron", prefix).as_bytes(), cron.as_bytes());
        batch.put_cf(
            cf_cron_queues,
            format!("{}_prompt", prefix).as_bytes(),
            prompt.as_bytes(),
        );
        batch.put_cf(
            cf_cron_queues,
            format!("{}_subprompt", prefix).as_bytes(),
            subprompt.as_bytes(),
        );
        batch.put_cf(cf_cron_queues, format!("{}_url", prefix).as_bytes(), url.as_bytes());
        batch.put_cf(
            cf_cron_queues,
            format!("{}_crawl_links", prefix).as_bytes(),
            crawl_links.to_string().as_bytes(),
        );

        let created_at = Utc::now().to_rfc3339();
        batch.put_cf(
            cf_cron_queues,
            format!("{}_created_at", prefix).as_bytes(),
            created_at.as_bytes(),
        );
        batch.put_cf(
            cf_cron_queues,
            format!("{}_agent_id", prefix).as_bytes(),
            llm_provider_id.as_bytes(),
        );

        // Commit the write batch
        self.db.write(batch)?;

        Ok(())
    }

    pub fn remove_cron_task(&self, profile: ShinkaiName, task_id: String) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;

        // Use Topic::CronQueues with standard prefixes for each task attribute
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let prefix = format!("{}_{}", profile_name, task_id);

        // Start a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Delete each attribute of the cron task with a unique prefix
        batch.delete_cf(cf_cron_queues, format!("{}_cron", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_prompt", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_subprompt", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_url", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_crawl_links", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_created_at", prefix).as_bytes());
        batch.delete_cf(cf_cron_queues, format!("{}_agent_id", prefix).as_bytes());

        // Commit the write batch
        self.db.write(batch)?;

        Ok(())
    }

    fn construct_cron_task_from_multiple_attributes(
        &self,
        task_id: String,
        attributes: HashMap<String, Vec<u8>>,
    ) -> Result<CronTask, ShinkaiDBError> {
        let mut cron_task = CronTask {
            task_id,
            cron: String::new(),
            prompt: String::new(),
            subprompt: String::new(),
            url: String::new(),
            crawl_links: false,
            created_at: String::new(),
            llm_provider_id: String::new(),
        };

        for (attribute, value) in attributes {
            match attribute.as_str() {
                "cron" => {
                    cron_task.cron = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for cron".to_string()))?
                }
                "prompt" => {
                    cron_task.prompt = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for prompt".to_string()))?
                }
                "subprompt" => {
                    cron_task.subprompt = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for subprompt".to_string()))?
                }
                "url" => {
                    cron_task.url = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for url".to_string()))?
                }
                "crawl_links" => {
                    cron_task.crawl_links = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for crawl_links".to_string()))?
                        .parse::<bool>()
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid bool for crawl_links".to_string()))?
                }
                "created_at" => {
                    cron_task.created_at = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for created_at".to_string()))?
                }
                "agent_id" => {
                    cron_task.llm_provider_id = String::from_utf8(value)
                        .map_err(|_| ShinkaiDBError::InvalidAttributeName("Invalid UTF-8 for agent_id".to_string()))?
                }
                _ => return Err(ShinkaiDBError::InvalidAttributeName(attribute)),
            }
        }

        Ok(cron_task)
    }

    pub fn get_all_cron_tasks_from_all_profiles(
        &self,
        node_name: ShinkaiName,
    ) -> Result<HashMap<String, Vec<(String, CronTask)>>, ShinkaiDBError> {
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        // Retrieve all profiles for the given node identity
        let profiles = self.get_all_profiles(node_name)?;

        let mut all_tasks = HashMap::new();

        for profile in profiles {
            // Assuming StandardIdentity has a method to get the profile name
            let profile_name = profile
                .full_identity_name
                .get_profile_name_string()
                .ok_or_else(|| ShinkaiDBError::InvalidAttributeName("Profile name not found".to_string()))?;

            // Construct the prefix for the cron tasks of this profile
            let prefix = format!("{}_{}", profile_name, "");
            let prefix_slice = prefix.as_bytes();

            // Temporary storage for task attributes before construction
            let mut task_attributes: HashMap<String, HashMap<String, Vec<u8>>> = HashMap::new();

            // Perform a prefix search for cron tasks of this profile
            let iter = self.db.prefix_iterator_cf(cf_cron_queues, prefix_slice);
            for result in iter {
                match result {
                    Ok((key, value)) => {
                        let key_str = String::from_utf8(key.to_vec()).unwrap();

                        // Extract task_id and attribute name from the key
                        if let Some((task_id_attribute, _)) = key_str.split_once('_') {
                            if let Some((task_id, attribute)) = task_id_attribute.rsplit_once('_') {
                                task_attributes
                                    .entry(task_id.to_string())
                                    .or_default()
                                    .insert(attribute.to_string(), value.to_vec());
                            }
                        }
                    }
                    Err(e) => return Err(ShinkaiDBError::from(e)), // Adjust this line according to how you convert rocksdb::Error to ShinkaiDBError
                }
            }

            // Construct CronTask objects from aggregated attributes and add them to all_tasks
            for (task_id, attributes) in task_attributes {
                let task = self.construct_cron_task_from_multiple_attributes(task_id.clone(), attributes)?;
                all_tasks
                    .entry(profile_name.clone())
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
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        let prefix = format!("{}_{}", profile_name, "");
        let prefix_slice = prefix.as_bytes();

        let mut tasks = HashMap::new();

        // Temporary storage for task attributes before construction
        let mut task_attributes: HashMap<String, HashMap<String, Vec<u8>>> = HashMap::new();

        // Perform a prefix search for cron tasks of this profile
        let iter = self.db.prefix_iterator_cf(cf_cron_queues, prefix_slice);
        for result in iter {
            match result {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();

                    // Extract task_id and attribute name from the key
                    if let Some((task_id_attribute, _)) = key_str.split_once('_') {
                        if let Some((task_id, attribute)) = task_id_attribute.rsplit_once('_') {
                            task_attributes
                                .entry(task_id.to_string())
                                .or_default()
                                .insert(attribute.to_string(), value.to_vec());
                        }
                    }
                }
                Err(e) => return Err(ShinkaiDBError::from(e)),
            }
        }

        // Construct CronTask objects from aggregated attributes and add them to tasks
        for (task_id, attributes) in task_attributes {
            let task = self.construct_cron_task_from_multiple_attributes(task_id.clone(), attributes)?;
            tasks.insert(task_id, task);
        }

        Ok(tasks)
    }

    pub fn get_cron_task(&self, profile: ShinkaiName, task_id: String) -> Result<CronTask, ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(ShinkaiDBError::InvalidProfileName("Invalid profile name".to_string()))?;
        let cf_cron_queues = self.get_cf_handle(Topic::CronQueues)?;

        // Construct the prefix for this specific task
        let prefix = format!("{}_{}", profile_name, task_id);

        // Temporary storage for task attributes before construction
        let mut attributes: HashMap<String, Vec<u8>> = HashMap::new();

        // Perform a prefix search for attributes of this task
        let iter = self.db.prefix_iterator_cf(cf_cron_queues, prefix.as_bytes());
        for result in iter {
            match result {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();

                    // Extract attribute name from the key, assuming the format "profileName_taskId_attributeName"
                    if let Some(attribute_name) = key_str.split('_').last() {
                        attributes.insert(attribute_name.to_string(), value.to_vec());
                    }
                }
                Err(e) => return Err(ShinkaiDBError::from(e)),
            }
        }

        // Construct the CronTask object from the aggregated attributes
        let task = self.construct_cron_task_from_multiple_attributes(task_id.clone(), attributes)?;

        Ok(task)
    }
}
