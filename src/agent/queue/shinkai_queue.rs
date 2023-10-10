use crossbeam_queue::ArrayQueue;
use rocksdb::DB;
use bincode::{serialize, deserialize};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use crate::agent::job_execution::JobForProcessing;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};

type Queue = Arc<ArrayQueue<JobForProcessing>>;
type Subscriber = mpsc::Sender<JobForProcessing>;

// Note(Nico): This 

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializedQueue {
    items: Vec<JobForProcessing>,
}

#[derive(Serialize, Deserialize)]
pub struct SharedJobQueueManager {
    queues: HashMap<String, Queue>,
    subscribers: HashMap<String, Vec<Subscriber>>,
    db: ShinkaiDB,
}

impl SharedJobQueueManager {
    pub fn new(db: Arc<ShinkaiDB>) -> Self {
        SharedJobQueueManager {
            queues: Arc::new(Mutex::new(HashMap::new())),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            db,
        }
    }

    fn get_queue(&self, key: &str) -> Result<Queue, ShinkaiDBError> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue) = queues.get(key) {
            return Ok(queue.clone());
        }
    
        match self.db.get_job_queues(key) {
            Ok(queue) => {
                queues.insert(key.to_string(), Arc::new(queue));
                Ok(queues.get(key).unwrap().clone())
            },
            Err(e) => Err(e),
        }
    }

    pub fn push(&self, key: &str, value: JobForProcessing) -> Result<(), ShinkaiDBError> {
        let queue = self.get_queue(key)?;
        queue.push(value.clone()).unwrap();
        self.db.persist_job_queues(key, &queue)?;

        // Notify subscribers
        if let Some(subs) = self.subscribers.lock().unwrap().get(key) {
            for sub in subs.iter() {
                sub.send(value.clone()).unwrap();
            }
        }
        Ok(())
    }

    pub fn pop(&self, key: &str) -> Result<Option<JobForProcessing>, ShinkaiDBError> {
        let queue = self.get_queue(key)?;
        let result = queue.pop();
        if result.is_some() {
            self.db.persist_job_queues(key, &queue)?;
        }
        Ok(result)
    }

    pub fn subscribe(&self, key: &str) -> mpsc::Receiver<JobForProcessing> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.lock().unwrap()
            .entry(key.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
        rx
    }
}

impl Clone for SharedJobQueueManager {
    fn clone(&self) -> Self {
        SharedJobQueueManager {
            queues: Arc::clone(&self.queues),
            subscribers: Arc::clone(&self.subscribers),
            db: Arc::clone(&self.db),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_manager() {
        let manager = SharedJobQueueManager::<String>::new("path_to_rocksdb");

        // Subscribe to notifications from "my_queue"
        let receiver = manager.subscribe("my_queue");
        let manager_clone = manager.clone(); 
        std::thread::spawn(move || {
            for msg in receiver.iter() {
                println!("Received (from subscriber): {}", msg);

                // Pop from the queue inside the subscriber thread
                if let Some(message) = manager_clone.pop("my_queue") {
                    println!("Popped (from subscriber): {}", message);
                }
            }
        });

        // Push to a queue
        manager.push("my_queue", "Hello".to_string());

        // Sleep to allow subscriber to process the message (just for this example)
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}