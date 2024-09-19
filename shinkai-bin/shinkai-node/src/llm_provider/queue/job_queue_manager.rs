use shinkai_db::db::db_errors::ShinkaiDBError;
use shinkai_db::db::ShinkaiDB;
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Weak};
use tokio::sync::{mpsc, Mutex};

type MutexQueue<T> = Arc<Mutex<Vec<T>>>;
type Subscriber<T> = mpsc::Sender<T>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct JobForProcessing {
    pub job_message: JobMessage,
    pub profile: ShinkaiName,
    pub date_created: String,
    // TODO: add a new optional field for callbacks
}

impl JobForProcessing {
    pub fn new(job_message: JobMessage, profile: ShinkaiName) -> Self {
        JobForProcessing {
            job_message,
            profile,
            date_created: Utc::now().to_rfc3339(),
        }
    }
}

impl PartialOrd for JobForProcessing {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JobForProcessing {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_created.cmp(&other.date_created)
    }
}

// Second Type
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OrdJsonValue(JsonValue);

impl Ord for OrdJsonValue {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_str = self.0.to_string();
        let other_str = other.0.to_string();
        self_str.cmp(&other_str)
    }
}

impl PartialOrd for OrdJsonValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for OrdJsonValue {}

impl PartialEq for OrdJsonValue {
    fn eq(&self, other: &Self) -> bool {
        let self_str = self.0.to_string();
        let other_str = other.0.to_string();
        self_str == other_str
    }
}

#[derive(Debug)]
pub struct JobQueueManager<T: Debug> {
    queues: Arc<Mutex<HashMap<String, MutexQueue<T>>>>,
    subscribers: Arc<Mutex<HashMap<String, Vec<Subscriber<T>>>>>,
    all_subscribers: Arc<Mutex<Vec<Subscriber<T>>>>,
    db: Weak<ShinkaiDB>,
    cf_name: String,
    prefix: Option<String>,
}

// Note: these are the ones that are kept in memory but the complete list is kept in the database
static BUFFER_SIZE: usize = 10;

impl<T: Clone + Send + 'static + DeserializeOwned + Serialize + Ord + Debug> JobQueueManager<T> {
    pub async fn new(db: Weak<ShinkaiDB>, cf_name: &str, prefix: Option<String>) -> Result<Self, ShinkaiDBError> {
        // Lock the db for safe access
        let db_arc = db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();

        // Call the get_all_queues method to get all queue data from the db
        match db_arc.get_all_queues(cf_name, prefix.clone()) {
            Ok(db_queues) => {
                // Initialize the queues field with Mutex-wrapped Vecs from the db data
                let manager_queues = db_queues
                    .into_iter()
                    .map(|(key, vec)| (key, Arc::new(Mutex::new(vec))))
                    .collect();

                // Return a new SharedJobQueueManager with the loaded queue data
                Ok(JobQueueManager {
                    queues: Arc::new(Mutex::new(manager_queues)),
                    subscribers: Arc::new(Mutex::new(HashMap::new())),
                    all_subscribers: Arc::new(Mutex::new(Vec::new())),
                    db: db.clone(),
                    cf_name: cf_name.to_string(),
                    prefix,
                })
            }
            Err(e) => Err(e),
        }
    }

    #[allow(dead_code)]
    async fn get_queue(&self, key: &str) -> Result<Vec<T>, ShinkaiDBError> {
        let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        db_arc.get_job_queues(&self.cf_name, key, self.prefix.clone())
    }

    pub async fn push(&mut self, key: &str, value: T) -> Result<(), ShinkaiDBError> {
        // Lock the Mutex to get mutable access to the HashMap
        let mut queues = self.queues.lock().await;

        // Ensure the specified key exists in the queues hashmap, initializing it with an empty queue if necessary
        let queue = queues
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));

        let mut guarded_queue = queue.lock().await;
        guarded_queue.push(value.clone());

        // Persist queue to the database
        let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        db_arc.persist_queue(&self.cf_name, key, &guarded_queue, self.prefix.clone())?;
        drop(db_arc);

        // Notify subscribers
        let subscribers = self.subscribers.lock().await;
        if let Some(subs) = subscribers.get(key) {
            for sub in subs.iter() {
                if sub.capacity() > 0 {
                    // Check if there's space in the buffer
                    let _ = sub.send(value.clone()).await;
                }
            }
        }

        // Notify subscribers to all keys
        let all_subscribers = self.all_subscribers.lock().await;
        for sub in all_subscribers.iter() {
            if sub.capacity() > 0 {
                // Check if there's space in the buffer
                let _ = sub.send(value.clone()).await;
            }
        }
        Ok(())
    }

    pub async fn dequeue(&mut self, key: &str) -> Result<Option<T>, ShinkaiDBError> {
        // Lock the Mutex to get mutable access to the HashMap
        let mut queues = self.queues.lock().await;

        // Ensure the specified key exists in the queues hashmap, initializing it with an empty queue if necessary
        let queue = queues
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));

        let mut guarded_queue = queue.lock().await;

        // Check if there's an element to dequeue, and remove it if so
        let result = if guarded_queue.first().is_some() {
            Some(guarded_queue.remove(0))
        } else {
            None
        };

        // Persist queue to the database
        let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        db_arc.persist_queue(&self.cf_name, key, &guarded_queue, self.prefix.clone())?;

        Ok(result)
    }

    pub async fn peek(&self, key: &str) -> Result<Option<T>, ShinkaiDBError> {
        let queues = self.queues.lock().await;
        if let Some(queue) = queues.get(key) {
            let guarded_queue = queue.lock().await;
            if let Some(first) = guarded_queue.first() {
                return Ok(Some(first.clone()));
            }
        }
        Ok(None)
    }

    pub async fn get_all_elements_interleave(&self) -> Result<Vec<T>, ShinkaiDBError> {
        let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db")?;
        let mut db_queues: HashMap<_, _> = db_arc.get_all_queues::<T>(&self.cf_name, self.prefix.clone())?;

        // Sort the keys based on the first element in each queue, falling back to key names
        let mut keys: Vec<_> = db_queues.keys().cloned().collect();
        keys.sort_by(|a, b| {
            let a_first = db_queues.get(a).and_then(|q| q.first());
            let b_first = db_queues.get(b).and_then(|q| q.first());
            match (a_first, b_first) {
                (Some(a), Some(b)) => a.cmp(b),
                _ => a.cmp(b),
            }
        });

        let mut all_elements = Vec::new();
        let mut indices: Vec<_> = vec![0; keys.len()];
        let mut added = true;

        while added {
            added = false;
            for (key, index) in keys.iter().zip(indices.iter_mut()) {
                if let Some(queue) = db_queues.get_mut(key) {
                    if let Some(element) = queue.get(*index) {
                        all_elements.push(element.clone());
                        *index += 1;
                        added = true;
                    }
                }
            }
        }

        Ok(all_elements)
    }

    #[allow(dead_code)]
    pub async fn subscribe(&self, key: &str) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        let mut subscribers = self.subscribers.lock().await;
        subscribers.entry(key.to_string()).or_insert_with(Vec::new).push(tx);
        rx
    }

    pub async fn subscribe_to_all(&self) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        let mut all_subscribers = self.all_subscribers.lock().await;
        all_subscribers.push(tx);
        rx
    }
}

impl<T: Clone + Send + 'static + Debug> Clone for JobQueueManager<T> {
    fn clone(&self) -> Self {
        JobQueueManager {
            queues: Arc::clone(&self.queues),
            subscribers: Arc::clone(&self.subscribers),
            all_subscribers: Arc::clone(&self.all_subscribers),
            db: self.db.clone(),
            cf_name: self.cf_name.clone(),
            prefix: self.prefix.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;
    use shinkai_db::db::Topic;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
    use std::{fs, path::Path};

    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_queue_manager() {
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);

        let mut manager = JobQueueManager::<JobForProcessing>::new(db_weak, Topic::AnyQueuesPrefixed.as_str(), None)
            .await
            .unwrap();

        // Subscribe to notifications from "my_queue"
        let mut receiver = manager.subscribe("job_id::123::false").await;
        let mut manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            if let Some(msg) = receiver.recv().await {
                // println!("Received (from subscriber): {:?}", msg);

                let results = manager_clone.get_all_elements_interleave().await.unwrap();
                // eprintln!("All elements: {:?}", results);

                // Dequeue from the queue inside the subscriber thread
                if let Ok(Some(message)) = manager_clone.dequeue("job_id::123::false").await {
                    println!("Dequeued (from subscriber): {:?}", message);

                    // Assert that the subscriber dequeued the correct message
                    assert_eq!(message, msg, "Dequeued message does not match received message");
                }

                // eprintln!("Dequeued (from subscriber): {:?}", msg);
                // Assert that the queue is now empty
                match manager_clone.dequeue("job_id::123::false").await {
                    Ok(None) => (),
                    Ok(Some(_)) => panic!("Queue is not empty!"),
                    Err(e) => panic!("Failed to dequeue from queue: {:?}", e),
                }
            }
        });

        // Push to a queue
        let job = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        manager.push("job_id::123::false", job.clone()).await.unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_queue_manager_consistency() {
        setup();
        let db_path = "db_tests/";
        let db_arc = Arc::new(ShinkaiDB::new(db_path).unwrap());
        let db_weak = Arc::downgrade(&db_arc);
        let mut manager =
            JobQueueManager::<JobForProcessing>::new(db_weak.clone(), Topic::AnyQueuesPrefixed.as_str(), None)
                .await
                .unwrap();

        // Push to a queue
        let job = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        let job2 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: "my content 2".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        manager.push("my_queue", job.clone()).await.unwrap();
        manager.push("my_queue", job2.clone()).await.unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Create a new manager and recover the state
        let mut new_manager =
            JobQueueManager::<JobForProcessing>::new(db_weak.clone(), Topic::AnyQueuesPrefixed.as_str(), None)
                .await
                .unwrap();

        // Try to pop the job from the queue using the new manager
        match new_manager.dequeue("my_queue").await {
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

        match new_manager.dequeue("my_queue").await {
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

    #[tokio::test]
    async fn test_queue_manager_with_jsonvalue() {
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);
        let mut manager = JobQueueManager::<OrdJsonValue>::new(db_weak, Topic::AnyQueuesPrefixed.as_str(), None)
            .await
            .unwrap();

        // Subscribe to notifications from "my_queue"
        let mut receiver = manager.subscribe("my_queue").await;
        let mut manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            if let Some(msg) = receiver.recv().await {
                println!("Received (from subscriber): {:?}", msg);

                // Dequeue from the queue inside the subscriber thread
                if let Ok(Some(message)) = manager_clone.dequeue("my_queue").await {
                    // Assert that the subscriber dequeued the correct message
                    assert_eq!(message, msg, "Dequeued message does not match received message");
                }

                // Assert that the queue is now empty
                match manager_clone.dequeue("my_queue").await {
                    Ok(None) => (),
                    Ok(Some(_)) => panic!("Queue is not empty!"),
                    Err(e) => panic!("Failed to dequeue from queue: {:?}", e),
                }
            }
        });

        // Push to a queue
        let job = JsonValue::String("my content".to_string());
        manager.push("my_queue", OrdJsonValue(job)).await.unwrap();

        // Sleep to allow subscriber to process the message (just for this example)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_get_all_elements_interleave() {
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);
        let mut manager = JobQueueManager::<JobForProcessing>::new(db_weak, Topic::AnyQueuesPrefixed.as_str(), None)
            .await
            .unwrap();

        // Create jobs
        let job_a1 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::a1::false".to_string(),
                content: "content a1".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        let job_a2 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::a2::false".to_string(),
                content: "content a2".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        let job_a3 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::a3::false".to_string(),
                content: "content a3".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );

        let job_b1 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::b1::false".to_string(),
                content: "content b1".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );

        let job_c1 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::c1::false".to_string(),
                content: "content c1".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );

        let job_c2 = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::c2::false".to_string(),
                content: "content c2".to_string(),
                files_inbox: "".to_string(),
                parent: None,
                workflow_code: None,
                workflow_name: None,
                sheet_job_data: None,
                callback: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );

        // Push jobs to queues
        manager.push("job_a", job_a1.clone()).await.unwrap();
        manager.push("job_a", job_a2.clone()).await.unwrap();
        manager.push("job_a", job_a3.clone()).await.unwrap();
        manager.push("job_b", job_b1.clone()).await.unwrap();
        manager.push("job_c", job_c1.clone()).await.unwrap();
        manager.push("job_c", job_c2.clone()).await.unwrap();

        // Get all elements interleaved
        let all_elements = manager.get_all_elements_interleave().await.unwrap();

        // Check if the elements are in the correct order
        assert_eq!(all_elements, vec![job_a1, job_b1, job_c1, job_a2, job_c2, job_a3]);
    }
}
