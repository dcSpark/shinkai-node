use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

type MutexQueue<T> = Arc<Mutex<Vec<T>>>;
type Subscriber<T> = mpsc::Sender<T>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct JobForProcessing {
    job_message: JobMessage,
    profile: ShinkaiName,
}

#[derive(Debug)]
pub struct JobQueueManager<T> {
    queues: HashMap<String, MutexQueue<T>>,
    subscribers: HashMap<String, Vec<Subscriber<T>>>,
    db: Arc<Mutex<ShinkaiDB>>,
}

impl<T: Clone + Send + 'static + DeserializeOwned + Serialize> JobQueueManager<T> {
    pub fn new(db: Arc<Mutex<ShinkaiDB>>) -> Result<Self, ShinkaiDBError> {
        // Lock the db for safe access
        let db_lock = db.lock().unwrap();

        // Call the get_all_queues method to get all queue data from the db
        match db_lock.get_all_queues() {
            Ok(db_queues) => {
                // Initialize the queues field with Mutex-wrapped Vecs from the db data
                let manager_queues = db_queues
                    .into_iter()
                    .map(|(key, vec)| (key, Arc::new(Mutex::new(vec))))
                    .collect();

                // Return a new SharedJobQueueManager with the loaded queue data
                Ok(JobQueueManager {
                    queues: manager_queues,
                    subscribers: HashMap::new(),
                    db: Arc::clone(&db),
                })
            }
            Err(e) => Err(e),
        }
    }

    fn get_queue(&self, key: &str) -> Result<Vec<T>, ShinkaiDBError> {
        let db = self.db.lock().unwrap();
        db.get_job_queues(key)
    }

    pub fn push(&mut self, key: &str, value: T) -> Result<(), ShinkaiDBError> {
        let queue = self
            .queues
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));

        let mut guarded_queue = queue.lock().unwrap();
        guarded_queue.push(value.clone());

        // Persist queue to the database
        let db = self.db.lock().unwrap();
        db.persist_queue(key, &guarded_queue)?;

        // Notify subscribers
        if let Some(subs) = self.subscribers.get(key) {
            for sub in subs.iter() {
                sub.send(value.clone()).unwrap();
            }
        }
        Ok(())
    }

    pub fn dequeue(&mut self, key: &str) -> Result<Option<T>, ShinkaiDBError> {
        // Ensure the specified key exists in the queues hashmap, initializing it with an empty queue if necessary
        let queue = self
            .queues
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));
        let mut guarded_queue = queue.lock().unwrap();

        // Check if there's an element to dequeue, and remove it if so
        let result = if guarded_queue.get(0).is_some() {
            Some(guarded_queue.remove(0))
        } else {
            None
        };

        // Persist queue to the database
        let db = self.db.lock().unwrap();
        db.persist_queue(key, &guarded_queue)?;

        Ok(result)
    }

    pub fn subscribe(&mut self, key: &str) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel();
        self.subscribers
            .entry(key.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
        rx
    }
}

impl<T: Clone + Send + 'static> Clone for JobQueueManager<T> {
    fn clone(&self) -> Self {
        JobQueueManager {
            queues: self.queues.clone(),
            subscribers: self.subscribers.clone(),
            db: Arc::clone(&self.db),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::{error::AgentError, queue::job_queue_manager_error::JobQueueManagerError};

    use super::*;
    use serde_json::Value as JsonValue;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
    use std::{fs, path::Path};

    #[test]
    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_queue_manager() {
        setup();
        let db = Arc::new(Mutex::new(ShinkaiDB::new("db_tests/").unwrap()));
        let mut manager = JobQueueManager::<JobForProcessing>::new(db).unwrap();

        // Subscribe to notifications from "my_queue"
        let receiver = manager.subscribe("my_queue");
        let mut manager_clone = manager.clone();
        let handle = std::thread::spawn(move || {
            for msg in receiver.iter() {
                println!("Received (from subscriber): {:?}", msg);

                // Dequeue from the queue inside the subscriber thread
                if let Ok(Some(message)) = manager_clone.dequeue("my_queue") {
                    println!("Dequeued (from subscriber): {:?}", message);

                    // Assert that the subscriber dequeued the correct message
                    assert_eq!(message, msg, "Dequeued message does not match received message");
                }

                eprintln!("Dequeued (from subscriber): {:?}", msg);
                // Assert that the queue is now empty
                match manager_clone.dequeue("my_queue") {
                    Ok(None) => (),
                    Ok(Some(_)) => panic!("Queue is not empty!"),
                    Err(e) => panic!("Failed to dequeue from queue: {:?}", e),
                }
                break;
            }
        });

        // Push to a queue
        let job = JobForProcessing {
            job_message: JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content".to_string(),
                files_inbox: "".to_string(),
            },
            profile: ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        };
        manager.push("my_queue", job.clone()).unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        std::thread::sleep(std::time::Duration::from_millis(500));

        handle.join().unwrap();
    }

    #[test]
    fn test_queue_manager_consistency() {
        setup();
        let db_path = "db_tests/";
        let db = Arc::new(Mutex::new(ShinkaiDB::new(db_path).unwrap()));
        let mut manager = JobQueueManager::<JobForProcessing>::new(Arc::clone(&db)).unwrap();

        // Push to a queue
        let job = JobForProcessing {
            job_message: JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content".to_string(),
                files_inbox: "".to_string(),
            },
            profile: ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        };
        let job2 = JobForProcessing {
            job_message: JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content 2".to_string(),
                files_inbox: "".to_string(),
            },
            profile: ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        };
        manager.push("my_queue", job.clone()).unwrap();
        manager.push("my_queue", job2.clone()).unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Create a new manager and recover the state
        let mut new_manager = JobQueueManager::<JobForProcessing>::new(Arc::clone(&db)).unwrap();

        // Try to pop the job from the queue using the new manager
        match new_manager.dequeue("my_queue") {
            Ok(Some(recovered_job)) => {
                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Info,
                    format!("Recovered job: {:?}", recovered_job).as_str(),
                );
                assert_eq!(recovered_job, job);
            }
            Ok(None) => panic!("No job found in the queue!"),
            Err(e) => panic!("Failed to pop job from queue: {:?}", e),
        }

        match new_manager.dequeue("my_queue") {
            Ok(Some(recovered_job)) => {
                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Info,
                    format!("Recovered job: {:?}", recovered_job).as_str(),
                );
                assert_eq!(recovered_job, job2);
            }
            Ok(None) => panic!("No job found in the queue!"),
            Err(e) => panic!("Failed to pop job from queue: {:?}", e),
        }
    }

    #[test]
    fn test_queue_manager_with_jsonvalue() {
        setup();
        let db = Arc::new(Mutex::new(ShinkaiDB::new("db_tests/").unwrap()));
        let mut manager = JobQueueManager::<Result<JsonValue, JobQueueManagerError>>::new(db).unwrap();

        // Subscribe to notifications from "my_queue"
        let receiver = manager.subscribe("my_queue");
        let mut manager_clone = manager.clone();
        let handle = std::thread::spawn(move || {
            for msg in receiver.iter() {
                println!("Received (from subscriber): {:?}", msg);

                // Dequeue from the queue inside the subscriber thread
                if let Ok(Some(message)) = manager_clone.dequeue("my_queue") {
                    println!("Dequeued (from subscriber): {:?}", message);

                    // Assert that the subscriber dequeued the correct message
                    assert_eq!(message, msg, "Dequeued message does not match received message");
                }

                eprintln!("Dequeued (from subscriber): {:?}", msg);
                // Assert that the queue is now empty
                match manager_clone.dequeue("my_queue") {
                    Ok(None) => (),
                    Ok(Some(_)) => panic!("Queue is not empty!"),
                    Err(e) => panic!("Failed to dequeue from queue: {:?}", e),
                }
                break;
            }
        });

        // Push to a queue
        let job = Ok(JsonValue::String("my content".to_string()));
        manager.push("my_queue", job.clone()).unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        std::thread::sleep(std::time::Duration::from_millis(500));
        handle.join().unwrap();
    }
}
