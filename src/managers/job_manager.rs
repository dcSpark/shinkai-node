use std::{collections::HashMap, sync::Arc, error::Error};

use tokio::sync::Mutex;
use uuid::Uuid;
use crate::{schemas::{inbox_name::InboxName, job_schemas::JobScope}, db::ShinkaiMessageDB};

pub trait JobLike {
    fn job_id(&self) -> &str;
    fn datetime_created(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn parent_agent_id(&self) -> &str;
    fn scope(&self) -> &JobScope;
    fn conversation_inbox_name(&self) -> &InboxName;
}

#[derive(Clone)]
pub struct Job {
    // based on uuid
    pub job_id: String,
    // Format: "20230702T20533481346" or Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    pub datetime_created: String, 
    // determines if the job is finished or not
    pub is_finished: bool,
    // identity of the parent agent. We just use a full identity name for simplicity
    pub parent_agent_id: String,
    // what storage buckets and/or documents are accessible to the LLM via vector search
    // and/or direct querying based off bucket name/key
    pub scope: JobScope,
    // an inbox where messages to the agent from the user and messages from the agent are stored,
    // enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    // A step history (an ordered list of all messages submitted to the LLM which triggered a step to execute,
    // including everything in the conversation inbox + any messages from the agent recursively calling itself or otherwise)
    pub step_history: Vec<String>,
}

impl JobLike for Job {
    fn job_id(&self) -> &str {
        &self.job_id
    }

    fn datetime_created(&self) -> &str {
        &self.datetime_created
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn parent_agent_id(&self) -> &str {
        &self.parent_agent_id
    }

    fn scope(&self) -> &JobScope {
        &self.scope
    }

    fn conversation_inbox_name(&self) -> &InboxName {
        &self.conversation_inbox_name
    }
}


// impl Job {
//     pub fn new(agent_id: String, scope: String, conversation_inbox: InboxName) -> Self {
//         // generating a unique job_id can be as simple as a UUID, or a more complex depending on your requirements
//         let job_id = format!("jobid_{}", uuid::Uuid::new_v4());

//         // TODO: verify that the agent_id exists?

//         Self {
//             job_id,
//             parent_agent_id: agent_id,
//             conversation_inbox,
//             step_history: Vec::new(),
//             scope,
//         }
//     }
// }

pub struct JobManager {
    jobs: HashMap<String, Job>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl JobManager {
    pub fn new(db: Arc<Mutex<ShinkaiMessageDB>>) -> Self {
        // let jobs = {
        //     let db = db.lock().await;
        //     db.load_all_jobs(local_node_name.clone())?
        // };

        Self {
            db,
            jobs: HashMap::new(), // TODO: read jobs from the db
        }
    }

    pub fn process_message(&mut self, message: String, job_id: Option<String>) -> Result<(), Box<dyn Error>> {
        match job_id {
            Some(id) => {
                let job = self.jobs.get_mut(&id).ok_or("Job id does not exist.")?;
                // process message for existing job
            },
            None => {
                // create a new job and process message
            },
        }
        Ok(())
    }

    pub fn execute_job(&self, job_id: String) -> Result<(), Box<dyn Error>> {
        let job = self.jobs.get(&job_id).ok_or("Job id does not exist.")?;
        // job execution logic here
        Ok(())
    }
}