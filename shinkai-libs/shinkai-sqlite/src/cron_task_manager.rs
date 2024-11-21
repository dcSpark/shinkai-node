use std::collections::HashMap;

use chrono::Utc;
use rusqlite::params;
use shinkai_message_primitives::schemas::{cron_task::CronTask, shinkai_name::ShinkaiName};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
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
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO cron_tasks (full_identity_name, task_id, cron, prompt, subprompt, url, crawl_links, created_at, llm_provider_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![profile.full_name, task_id, cron, prompt, subprompt, url, crawl_links, Utc::now().to_rfc3339(), llm_provider_id],
        )?;
        Ok(())
    }

    pub fn remove_cron_task(&self, profile: ShinkaiName, task_id: String) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM cron_tasks WHERE full_identity_name = ?1 AND task_id = ?2",
            params![profile.full_name, task_id],
        )?;
        Ok(())
    }

    pub fn get_all_cron_tasks_from_all_profiles(
        &self,
        _node_name: ShinkaiName,
    ) -> Result<HashMap<String, Vec<(String, CronTask)>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT full_identity_name, task_id, cron, prompt, subprompt, url, crawl_links, created_at, llm_provider_id FROM cron_tasks")?;
        let mut rows = stmt.query([])?;

        let mut result: HashMap<String, Vec<(String, CronTask)>> = HashMap::new();
        while let Some(row) = rows.next()? {
            let full_identity_name: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let cron: String = row.get(2)?;
            let prompt: String = row.get(3)?;
            let subprompt: String = row.get(4)?;
            let url: String = row.get(5)?;
            let crawl_links: bool = row.get(6)?;
            let created_at: String = row.get(7)?;
            let llm_provider_id: String = row.get(8)?;

            let cron_task = CronTask {
                task_id: task_id.clone(),
                cron,
                prompt,
                subprompt,
                url,
                crawl_links,
                created_at,
                llm_provider_id,
            };

            if let Some(tasks) = result.get_mut(&full_identity_name) {
                tasks.push((task_id, cron_task));
            } else {
                result.insert(full_identity_name, vec![(task_id, cron_task)]);
            }
        }

        Ok(result)
    }

    pub fn get_all_cron_tasks_for_profile(
        &self,
        profile: ShinkaiName,
    ) -> Result<HashMap<String, CronTask>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT task_id, cron, prompt, subprompt, url, crawl_links, created_at, llm_provider_id FROM cron_tasks WHERE full_identity_name = ?1")?;
        let mut rows = stmt.query(params![profile.full_name])?;

        let mut result: HashMap<String, CronTask> = HashMap::new();
        while let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let cron: String = row.get(1)?;
            let prompt: String = row.get(2)?;
            let subprompt: String = row.get(3)?;
            let url: String = row.get(4)?;
            let crawl_links: bool = row.get(5)?;
            let created_at: String = row.get(6)?;
            let llm_provider_id: String = row.get(7)?;

            let cron_task = CronTask {
                task_id: task_id.clone(),
                cron,
                prompt,
                subprompt,
                url,
                crawl_links,
                created_at,
                llm_provider_id,
            };

            result.insert(task_id, cron_task);
        }

        Ok(result)
    }

    pub fn get_cron_task(&self, profile: ShinkaiName, task_id: String) -> Result<CronTask, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT cron, prompt, subprompt, url, crawl_links, created_at, llm_provider_id FROM cron_tasks WHERE full_identity_name = ?1 AND task_id = ?2")?;
        let mut rows = stmt.query(params![profile.full_name, task_id])?;

        if let Some(row) = rows.next()? {
            let cron: String = row.get(0)?;
            let prompt: String = row.get(1)?;
            let subprompt: String = row.get(2)?;
            let url: String = row.get(3)?;
            let crawl_links: bool = row.get(4)?;
            let created_at: String = row.get(5)?;
            let llm_provider_id: String = row.get(6)?;

            Ok(CronTask {
                task_id,
                cron,
                prompt,
                subprompt,
                url,
                crawl_links,
                created_at,
                llm_provider_id,
            })
        } else {
            Err(SqliteManagerError::DataNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_add_and_get_cron_task() {
        let db = setup_test_db();
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
        let task_id = "test_task_id".to_string();
        let cron = "0 0 * * *".to_string();
        let prompt = "test_prompt".to_string();
        let subprompt = "test_subprompt".to_string();
        let url = "https://example.com".to_string();
        let crawl_links = true;
        let llm_provider_id = "test_llm_provider_id".to_string();

        db.add_cron_task(
            profile.clone(),
            task_id.clone(),
            cron.clone(),
            prompt.clone(),
            subprompt.clone(),
            url.clone(),
            crawl_links,
            llm_provider_id.clone(),
        )
        .unwrap();

        let cron_task = db.get_cron_task(profile.clone(), task_id.clone()).unwrap();
        assert_eq!(cron_task.task_id, task_id);
        assert_eq!(cron_task.cron, cron);
        assert_eq!(cron_task.prompt, prompt);
        assert_eq!(cron_task.subprompt, subprompt);
        assert_eq!(cron_task.url, url);
        assert_eq!(cron_task.crawl_links, crawl_links);
        assert_eq!(cron_task.llm_provider_id, llm_provider_id);
    }

    #[test]
    fn test_get_all_cron_tasks_for_profile() {
        let db = setup_test_db();
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
        let task_id = "test_task_id".to_string();
        let cron = "0 0 * * *".to_string();
        let prompt = "test_prompt".to_string();
        let subprompt = "test_subprompt".to_string();
        let url = "https://example.com".to_string();
        let crawl_links = true;
        let llm_provider_id = "test_llm_provider_id".to_string();

        db.add_cron_task(
            profile.clone(),
            task_id.clone(),
            cron.clone(),
            prompt.clone(),
            subprompt.clone(),
            url.clone(),
            crawl_links,
            llm_provider_id.clone(),
        )
        .unwrap();

        let cron_tasks = db.get_all_cron_tasks_for_profile(profile.clone()).unwrap();
        assert_eq!(cron_tasks.len(), 1);

        let cron_task = cron_tasks.get(&task_id).unwrap();
        assert_eq!(cron_task.task_id, task_id);
        assert_eq!(cron_task.cron, cron);
        assert_eq!(cron_task.prompt, prompt);
        assert_eq!(cron_task.subprompt, subprompt);
        assert_eq!(cron_task.url, url);
        assert_eq!(cron_task.crawl_links, crawl_links);
        assert_eq!(cron_task.llm_provider_id, llm_provider_id);
    }

    #[test]
    fn test_get_all_cron_tasks_from_all_profiles() {
        let db = setup_test_db();
        let profile1 = ShinkaiName::new("@@test_user1.shinkai/main".to_string()).unwrap();
        let profile2 = ShinkaiName::new("@@test_user2.shinkai/main".to_string()).unwrap();
        let task_id1 = "test_task_id1".to_string();
        let task_id2 = "test_task_id2".to_string();
        let cron = "0 0 * * *".to_string();
        let prompt = "test_prompt".to_string();
        let subprompt = "test_subprompt".to_string();
        let url = "https://example.com".to_string();
        let crawl_links = true;
        let llm_provider_id = "test_llm_provider_id".to_string();

        db.add_cron_task(
            profile1.clone(),
            task_id1.clone(),
            cron.clone(),
            prompt.clone(),
            subprompt.clone(),
            url.clone(),
            crawl_links,
            llm_provider_id.clone(),
        )
        .unwrap();

        db.add_cron_task(
            profile2.clone(),
            task_id2.clone(),
            cron.clone(),
            prompt.clone(),
            subprompt.clone(),
            url.clone(),
            crawl_links,
            llm_provider_id.clone(),
        )
        .unwrap();

        let cron_tasks = db.get_all_cron_tasks_from_all_profiles(profile1.clone()).unwrap();
        assert_eq!(cron_tasks.len(), 2);

        let cron_task1 = cron_tasks.get(&profile1.full_name).unwrap();
        assert_eq!(cron_task1.len(), 1);
        let cron_task1 = cron_task1.get(0).unwrap();
        assert_eq!(cron_task1.0, task_id1);
        assert_eq!(cron_task1.1.task_id, task_id1);
        assert_eq!(cron_task1.1.cron, cron);
        assert_eq!(cron_task1.1.prompt, prompt);
        assert_eq!(cron_task1.1.subprompt, subprompt);
        assert_eq!(cron_task1.1.url, url);
        assert_eq!(cron_task1.1.crawl_links, crawl_links);
        assert_eq!(cron_task1.1.llm_provider_id, llm_provider_id);

        let cron_task2 = cron_tasks.get(&profile2.full_name).unwrap();
        assert_eq!(cron_task2.len(), 1);
        let cron_task2 = cron_task2.get(0).unwrap();
        assert_eq!(cron_task2.0, task_id2);
        assert_eq!(cron_task2.1.task_id, task_id2);
        assert_eq!(cron_task2.1.cron, cron);
        assert_eq!(cron_task2.1.prompt, prompt);
        assert_eq!(cron_task2.1.subprompt, subprompt);
        assert_eq!(cron_task2.1.url, url);
        assert_eq!(cron_task2.1.crawl_links, crawl_links);
        assert_eq!(cron_task2.1.llm_provider_id, llm_provider_id);
    }

    #[test]
    fn test_remove_cron_task() {
        let db = setup_test_db();
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
        let task_id = "test_task_id".to_string();
        let cron = "0 0 * * *".to_string();
        let prompt = "test_prompt".to_string();
        let subprompt = "test_subprompt".to_string();
        let url = "https://example.com".to_string();
        let crawl_links = true;
        let llm_provider_id = "test_llm_provider_id".to_string();

        db.add_cron_task(
            profile.clone(),
            task_id.clone(),
            cron.clone(),
            prompt.clone(),
            subprompt.clone(),
            url.clone(),
            crawl_links,
            llm_provider_id.clone(),
        )
        .unwrap();

        let cron_task = db.get_cron_task(profile.clone(), task_id.clone()).unwrap();
        assert_eq!(cron_task.task_id, task_id);

        db.remove_cron_task(profile.clone(), task_id.clone()).unwrap();

        let cron_tasks = db.get_all_cron_tasks_for_profile(profile.clone()).unwrap();
        assert_eq!(cron_tasks.len(), 0);
    }
}
