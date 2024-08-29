use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::llm_provider::execution::prompts::subprompts::SubPromptType::{Assistant, User};
use std::{fs, path::Path};
use tokio::time::{sleep, Duration};

use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn create_new_job(db: &mut ShinkaiDB, job_id: String, agent_id: String, scope: JobScope) {
    match db.create_new_job(job_id, agent_id, scope, false, None) {
        Ok(_) => (),
        Err(e) => panic!("Failed to create a new job: {}", e),
    }
}

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    timestamp: String,
) -> ShinkaiMessage {
    let inbox_name = InboxName::get_job_inbox_name_from_params("test_job".to_string()).unwrap();

    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            inbox_name_value,
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            timestamp,
        )
        .build()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use shinkai_message_primitives::{
        schemas::inbox_name::InboxName,
        shinkai_message::shinkai_message_schemas::JobMessage,
        shinkai_utils::signatures::clone_signature_secret_key,
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, job_scope::JobScope,
            shinkai_logging::init_default_tracing, shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::unsafe_deterministic_signature_keypair,
        },
    };
    use shinkai_node::{db::db_errors::ShinkaiDBError, llm_provider::execution::prompts::subprompts::SubPrompt};
    use shinkai_vector_resources::utils::hash_string;

    use super::*;

    #[test]
    fn test_create_new_job() {
        
        setup();
        let job_id = "job1".to_string();
        let agent_id = "agent1".to_string();
        let scope = JobScope::new_default();
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
        assert_eq!(job.parent_llm_provider_id, agent_id);
        assert!(!job.is_finished);
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
            eprintln!("job_id: {}", job_id.clone());
            let scope = JobScope::new_default();
            create_new_job(&mut shinkai_db, job_id, agent_id.clone(), scope);
        }

        eprintln!("agent_id: {}", agent_id.clone());

        // Get all jobs for the agent
        let jobs = shinkai_db.get_agent_jobs(agent_id.clone()).unwrap();

        // Assert that all jobs are returned
        assert_eq!(jobs.len(), 5);

        // Additional check that all jobs have correct agent_id
        for job in jobs {
            assert_eq!(job.parent_llm_provider_id(), &agent_id);
        }
    }

    #[test]
    fn test_change_job_agent() {
        
        setup();
        let job_id = "job_to_change_agent".to_string();
        let initial_agent_id = "initial_agent".to_string();
        let new_agent_id = "new_agent".to_string();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&initial_agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job with the initial agent
        create_new_job(&mut shinkai_db, job_id.clone(), initial_agent_id.clone(), scope);

        // Change the agent of the job
        shinkai_db.change_job_llm_provider(&job_id, &new_agent_id).unwrap();

        // Retrieve the job and check that the agent has been updated
        let job = shinkai_db.get_job(&job_id).unwrap();
        assert_eq!(job.parent_llm_provider_id, new_agent_id);

        // Check that the job is listed under the new agent
        let new_agent_jobs = shinkai_db.get_agent_jobs(new_agent_id.clone()).unwrap();
        let job_ids: Vec<String> = new_agent_jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(job_ids.contains(&job_id));

        // Check that the job is no longer listed under the initial agent
        let initial_agent_jobs = shinkai_db.get_agent_jobs(initial_agent_id.clone()).unwrap();
        let initial_job_ids: Vec<String> = initial_agent_jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(!initial_job_ids.contains(&job_id));
    }

    #[test]
    fn test_update_job_to_finished() {
        
        setup();
        let job_id = "job3".to_string();
        let agent_id = "agent3".to_string();
        // let inbox_name =
        //     InboxName::new("inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true".to_string())
        //         .unwrap();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        // Update job to finished
        shinkai_db.update_job_to_finished(&job_id.clone()).unwrap();

        // Retrieve the job and check that is_finished is set to true
        let job = shinkai_db.get_job(&job_id.clone()).unwrap();
        assert!(job.is_finished);
    }

    #[tokio::test]
    async fn test_update_step_history() {
        
        setup();
        let job_id = "test_job".to_string();
        let agent_id = "agent4".to_string();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        let message = generate_message_with_text(
            "Hello World".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        // Insert the ShinkaiMessage into the database
        shinkai_db.unsafe_insert_inbox_message(&message, None, None).await.unwrap();

        // Update step history
        shinkai_db
            .add_step_history(
                job_id.clone(),
                "What is 10 + 25".to_string(),
                "The answer is 35".to_string(),
                None,
            )
            .unwrap();
        sleep(Duration::from_millis(10)).await;
        shinkai_db
            .add_step_history(
                job_id.clone(),
                "2) What is 10 + 25".to_string(),
                "2) The answer is 35".to_string(),
                None,
            )
            .unwrap();

        // Retrieve the job and check that step history is updated
        let job = shinkai_db.get_job(&job_id.clone()).unwrap();
        assert_eq!(job.step_history.len(), 2);
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

        match shinkai_db.update_job_to_finished(&job_id.clone()) {
            Ok(_) => panic!("Expected an error when updating a non-existent job"),
            Err(e) => assert_eq!(e, ShinkaiDBError::DataNotFound),
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
            // let inbox_name =
            //     InboxName::new("inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true".to_string())
            //         .unwrap();
            // let inbox_names = vec![inbox_name];
            // let documents = vec!["document1".to_string(), "document2".to_string()];

            let scope = JobScope::new_default();
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

    #[tokio::test]
    async fn test_job_inbox_empty() {
        
        setup();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone().to_string()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        // Check if the job inbox is empty after creating a new job
        assert!(shinkai_db.is_job_inbox_empty(&job_id).unwrap());

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            "something".to_string(),
            "".to_string(),
            placeholder_signature_sk,
            "@@node1.shinkai".to_string(),
            "@@node1.shinkai".to_string(),
        )
        .unwrap();

        // Add a message to the job
        let _ = shinkai_db
            .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None, None)
            .await;

        // Check if the job inbox is not empty after adding a message
        assert!(!shinkai_db.is_job_inbox_empty(&job_id).unwrap());
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure() {
        
        setup();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone().to_string()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            └── 3
         */
        for i in 1..=4 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 => parent_message_hash_2.clone(),
                _ => None,
            };

            // Add a message to the job
            let _ = shinkai_db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(shinkai_message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(shinkai_message.calculate_message_hash_for_pagination());
            }
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!shinkai_db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = shinkai_db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        let message_content_3 = last_messages_inbox[1][1].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_4 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_4: JobMessage = serde_json::from_str(&message_content_4).unwrap();
        assert_eq!(job_message_4.content, "Hello World 4".to_string());
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure_with_step_history_and_execution_context() {
        
        setup();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone().to_string()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            └── 3
         */
        let mut current_level = 0;
        let mut results = Vec::new();
        for i in 1..=4 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();

            let parent_hash: Option<String> = match i {
                2 | 3 => {
                    current_level += 1;
                    parent_message_hash.clone()
                }
                4 => {
                    results.pop();
                    parent_message_hash_2.clone()
                }
                _ => None,
            };

            // Add a message to the job
            let _ = shinkai_db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;

            // Add a step history
            let result = format!("Result {}", i);
            shinkai_db
                .add_step_history(
                    job_id.clone(),
                    format!("Step {} Level {}", i, current_level),
                    result.clone(),
                    None,
                )
                .unwrap();

            // Add the result to the results vector
            results.push(result);

            // Set job execution context
            let mut execution_context = HashMap::new();
            execution_context.insert("context".to_string(), results.join(", "));
            shinkai_db
                .set_job_execution_context(job_id.clone(), execution_context, None)
                .unwrap();

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(shinkai_message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(shinkai_message.calculate_message_hash_for_pagination());
            }
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!shinkai_db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = shinkai_db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        let message_content_3 = last_messages_inbox[1][1].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_4 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_4: JobMessage = serde_json::from_str(&message_content_4).unwrap();
        assert_eq!(job_message_4.content, "Hello World 4".to_string());

        // Check the step history and execution context
        let job = shinkai_db.get_job(&job_id.clone()).unwrap();
        eprintln!("job execution context: {:?}", job.execution_context);

        // Check the execution context
        assert_eq!(
            job.execution_context.get("context").unwrap(),
            "Result 1, Result 2, Result 4"
        );

        // Check the step history
        let step1 = &job.step_history[0];
        let step2 = &job.step_history[1];
        let step4 = &job.step_history[2];

        assert_eq!(
            step1.step_revisions[0].sub_prompts[0],
            SubPrompt::Content(User, "Step 1 Level 0".to_string(), 100)
        );
        assert_eq!(
            step1.step_revisions[0].sub_prompts[1],
            SubPrompt::Content(Assistant, "Result 1".to_string(), 100)
        );

        assert_eq!(
            step2.step_revisions[0].sub_prompts[0],
            SubPrompt::Content(User, "Step 2 Level 1".to_string(), 100)
        );
        assert_eq!(
            step2.step_revisions[0].sub_prompts[1],
            SubPrompt::Content(Assistant, "Result 2".to_string(), 100)
        );

        assert_eq!(
            step4.step_revisions[0].sub_prompts[0],
            SubPrompt::Content(User, "Step 4 Level 2".to_string(), 100)
        );
        assert_eq!(
            step4.step_revisions[0].sub_prompts[1],
            SubPrompt::Content(Assistant, "Result 4".to_string(), 100)
        );
    }

    #[tokio::test]
    async fn test_insert_steps_with_simple_tree_structure() {
        
        setup();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let job_id = "test_job";
        let db_path = "db_tests/test_job";
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        let mut shinkai_db = ShinkaiDB::new(db_path).unwrap();

        create_new_job(&mut shinkai_db, job_id.to_string(), agent_id.clone(), scope);

        eprintln!("Inserting steps...\n\n");
        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   └── 4
            └── 3
         */
        for i in 1..=4 {
            let user_message = format!("User message {}", i);
            let agent_response = format!("Agent response {}", i);

            // Generate the ShinkaiMessage
            let message = generate_message_with_text(
                format!("Hello World {}", i),
                node1_encryption_sk.clone(),
                clone_signature_secret_key(&node1_identity_sk),
                node1_encryption_pk,
                node1_subidentity_name.to_string(),
                node1_identity_name.to_string(),
                format!("2023-07-02T20:53:34.81{}Z", i),
            );

            eprintln!("Message: {:?}", message);

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 => parent_message_hash_2.clone(),
                _ => None,
            };

            // Insert the ShinkaiMessage into the database
            shinkai_db
                .unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
                .await
                .unwrap();

            shinkai_db
                .add_step_history(job_id.to_string(), user_message, agent_response, None)
                .unwrap();

            // Update the parent message hash according to the tree structure
            if i == 1 {
                parent_message_hash = Some(message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
            }
        }

        eprintln!("\n\n Getting messages...");
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string()).unwrap();
        let last_messages_inbox = shinkai_db
            .get_last_messages_from_inbox(inbox_name.to_string(), 3, None)
            .unwrap();

        let last_messages_content: Vec<Vec<String>> = last_messages_inbox
            .iter()
            .map(|message_array| {
                message_array
                    .iter()
                    .map(|message| message.clone().get_message_content().unwrap())
                    .collect()
            })
            .collect();

        eprintln!("Messages: {:?}", last_messages_content);

        eprintln!("\n\n Getting steps...");

        let step_history = shinkai_db.get_step_history(job_id, true).unwrap().unwrap();

        let step_history_content: Vec<String> = step_history
            .iter()
            .map(|step| {
                let user_message = match &step.step_revisions[0].sub_prompts[0] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                };
                let agent_response = match &step.step_revisions[0].sub_prompts[1] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                };
                format!("{} {}", user_message, agent_response)
            })
            .collect();

        eprintln!("Step history: {:?}", step_history_content);

        assert_eq!(step_history.len(), 3);

        // Check the content of the steps
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[0].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[0].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 1 Agent response 1".to_string()
        );
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[1].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[1].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 2 Agent response 2".to_string()
        );
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[2].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[2].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Content(_, text, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 4 Agent response 4".to_string()
        );
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure_with_invalid_date() {
        
        setup();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();
        let db_path = format!("db_tests/{}", hash_string(&agent_id.clone().to_string()));
        let mut shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Create a new job
        create_new_job(&mut shinkai_db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        // Create the messages
        let mut messages = Vec::new();
        for i in [1, 3, 2].iter() {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();
            messages.push(shinkai_message);

            sleep(Duration::from_millis(10)).await;
        }

        /*
        The tree that we are creating looks like:
            1
            ├── 2
                └── 3 (older date than 2. it should'nt fail)
         */

        // Add the messages to the job in a specific order to simulate an invalid date scenario
        for i in [0, 2, 1].iter() {
            let _result = shinkai_db
                .add_message_to_job_inbox(&job_id.clone(), &messages[*i], None, None)
                .await;
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!shinkai_db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = shinkai_db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 1);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_3 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());
    }
}
