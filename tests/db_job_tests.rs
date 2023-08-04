use std::{fs, path::Path};

use async_std::task;
use rocksdb::{Error, Options, WriteBatch};
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::JobScope;
use shinkai_node::{
    db::ShinkaiDB,
};

fn create_new_job(db: &mut ShinkaiDB, job_id: String, agent_id: String, scope: JobScope) {
    match db.create_new_job(job_id, agent_id, scope) {
        Ok(_) => (),
        Err(e) => panic!("Failed to create a new job: {}", e),
    }
}

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use shinkai_message_wasm::{shinkai_message::shinkai_message_schemas::JobScope, shinkai_utils::utils::hash_string, schemas::inbox_name::InboxName};
    use shinkai_node::{db::db_errors::ShinkaiDBError, managers::agent};

    use super::*;

    #[test]
    fn test_create_new_job() {
        setup();
        let job_id = "job1".to_string();
        let agent_id = "agent1".to_string();
        let inbox_name =
            InboxName::new("inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string())
                .unwrap();
        let scope = JobScope::new(Some(vec![inbox_name]), None);
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone().to_string()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        // Retrieve all jobs
        let jobs = shinkai_db.get_all_jobs().unwrap();

        // Check if the job exists
        let job_ids: Vec<String> = jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(job_ids.contains(&job_id));

        // Check that the job has the correct properties
        let job = shinkai_db.get_job(&job_id).unwrap();
        assert_eq!(job.job_id, job_id);
        assert_eq!(job.parent_agent_id, agent_id);
        assert_eq!(job.scope.buckets.len(), 1);
        assert_eq!(job.scope.documents.len(), 0);
        assert_eq!(job.is_finished, false);
    }

    #[test]
    fn test_get_agent_jobs() {
        setup();
        let agent_id = "agent2".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create new jobs for the agent
        for i in 1..=5 {
            let job_id = format!("job{}", i);
            let inbox_name =
                InboxName::new("inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string())
                    .unwrap();
            let inbox_names = vec![inbox_name];
            let documents = vec!["document1".to_string(), "document2".to_string()];

            let scope = JobScope::new(Some(inbox_names), Some(documents));
            create_new_job(&mut shinkai_db, job_id, agent_id.clone(), scope);
        }

        // Get all jobs for the agent
        let jobs = shinkai_db.get_agent_jobs(agent_id.clone()).unwrap();

        // Assert that all jobs are returned
        assert_eq!(jobs.len(), 5);

        // Additional check that all jobs have correct agent_id
        for job in jobs {
            assert_eq!(job.parent_agent_id(), &agent_id);
        }
    }

    #[test]
    fn test_update_job_to_finished() {
        setup();
        let job_id = "job3".to_string();
        let agent_id = "agent3".to_string();
        let inbox_name =
            InboxName::new("inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string())
                .unwrap();
        let scope = JobScope::new(Some(vec![inbox_name]), None);
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        // Update job to finished
        shinkai_db.update_job_to_finished(job_id.clone()).unwrap();

        // Retrieve the job and check that is_finished is set to true
        let job = shinkai_db.get_job(&job_id.clone()).unwrap();
        assert_eq!(job.is_finished, true);
    }

    #[test]
    fn test_update_step_history() {
        setup();
        let job_id = "job4".to_string();
        let agent_id = "agent4".to_string();
        let inbox_name =
            InboxName::new("inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string())
                .unwrap();
        let scope = JobScope::new(Some(vec![inbox_name]), None);
        let step = "step1".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        // Update step history
        shinkai_db.add_step_history(job_id.clone(), step.clone()).unwrap();

        // Retrieve the job and check that step history is updated
        let job = shinkai_db.get_job(&job_id.clone()).unwrap();
        assert_eq!(job.step_history.last().unwrap(), &step);
    }

    #[test]
    fn test_get_non_existent_job() {
        setup();
        let job_id = "non_existent_job".to_string();
        let agent_id = "agent".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id));
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        match shinkai_db.get_job(&job_id) {
            Ok(_) => panic!("Expected an error when getting a non-existent job"),
            Err(e) => assert_eq!(e, ShinkaiDBError::DataNotFound),
        }
    }

    #[test]
    fn test_get_agent_jobs_none_exist() {
        setup();
        let agent_id = "agent_without_jobs".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Attempt to get all jobs for the agent
        let jobs_result = shinkai_db.get_agent_jobs(agent_id.clone());

        match jobs_result {
            Ok(jobs) => {
                // If we got a result, assert that no jobs are returned
                assert_eq!(jobs.len(), 0);
            }
            Err(e) => {
                // If we got an error, check if it's because the agent doesn't exist
                assert_eq!(e, ShinkaiDBError::ColumnFamilyNotFound(format!("agentid_{}", agent_id)));
            }
        }
    }

    #[test]
    fn test_update_non_existent_job() {
        setup();
        let job_id = "non_existent_job".to_string();
        let agent_id = "agent".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id));
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        match shinkai_db.update_job_to_finished(job_id.clone()) {
            Ok(_) => panic!("Expected an error when updating a non-existent job"),
            Err(e) => assert_eq!(e, ShinkaiDBError::ProfileNameNonExistent(format!("jobtopic_{}", job_id))),
        }
    }

    #[test]
    fn test_get_agent_jobs_multiple_jobs() {
        setup();
        let agent_id = "agent5".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create new jobs for the agent
        for i in 1..=5 {
            let job_id = format!("job{}", i);
            let inbox_name =
                InboxName::new("inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string())
                    .unwrap();
            let inbox_names = vec![inbox_name];
            let documents = vec!["document1".to_string(), "document2".to_string()];

            let scope = JobScope::new(Some(inbox_names), Some(documents));
            create_new_job(&mut shinkai_db, job_id, agent_id.clone(), scope);
        }

        // Get all jobs for the agent
        let jobs = shinkai_db.get_agent_jobs(agent_id.clone()).unwrap();

        // Assert that all jobs are returned
        assert_eq!(jobs.len(), 5);

        // Additional check that all jobs have correct agent_id and they are unique
        let unique_jobs: HashSet<String> = jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert_eq!(unique_jobs.len(), 5);
    }
}
