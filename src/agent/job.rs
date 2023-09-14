use shinkai_message_primitives::{schemas::inbox_name::InboxName, shinkai_message::shinkai_message_schemas::JobScope};

pub type JobId = String;

pub trait JobLike: Send + Sync {
    fn job_id(&self) -> &str;
    fn datetime_created(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn parent_agent_id(&self) -> &str;
    fn scope(&self) -> &JobScope;
    fn conversation_inbox_name(&self) -> &InboxName;
}

// Todo: Add a persistent_context: String
#[derive(Clone, Debug)]
pub struct Job {
    // Based on uuid
    pub job_id: String,
    // Format: "20230702T20533481346" or Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    pub datetime_created: String,
    // Marks if the job is finished or not
    pub is_finished: bool,
    // Identity of the parent agent. We just use a full identity name for simplicity
    pub parent_agent_id: String,
    // What VectorResources the Job has access to when performing vector searches
    pub scope: JobScope,
    // An inbox where messages to the agent from the user and messages from the agent are stored,
    // enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    // The job's step history (an ordered list of all prompts/outputs from LLM inferencing when processing steps)
    pub step_history: Vec<String>,
    // An ordered list of the latest messages sent to the job which are yet to be processed
    pub unprocessed_messages: Vec<String>,
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
