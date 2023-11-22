use super::execution::job_prompts::Prompt;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::{schemas::inbox_name::InboxName, shinkai_utils::job_scope::JobScope};
use std::collections::HashMap;
use std::fmt;

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
    /// Based on uuid
    pub job_id: String,
    /// Format: "20230702T20533481346" or Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    pub datetime_created: String,
    /// Marks if the job is finished or not
    pub is_finished: bool,
    /// Identity of the parent agent. We just use a full identity name for simplicity
    pub parent_agent_id: String,
    /// What VectorResources the Job has access to when performing vector searches
    pub scope: JobScope,
    /// An inbox where messages to the agent from the user and messages from the agent are stored,
    /// enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    /// The job's step history (an ordered list of all prompts/outputs from LLM inferencing when processing steps)
    pub step_history: Vec<JobStepResult>,
    /// An ordered list of the latest messages sent to the job which are yet to be processed
    pub unprocessed_messages: Vec<String>,
    // /// A hashmap which holds a bunch of labeled values which were generated as output from the latest Job step
    pub execution_context: HashMap<String, String>,
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

/// Result from a Job step, holding user's message and Agent's response.
/// Includes revisions interface in case of edits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStepResult {
    /// Datetime of the first message sent from the user that triggered this Job Step
    pub initial_message_datetime: String,
    /// List of Prompts that hold User->System sub prompt pairs that denote what the user
    /// asked, and what the Agent finally responded with. These are the revisions for this
    /// single step, meaning that if this list has more than one prompt, later ones denote
    /// edits which were made off of the original message.
    pub step_revisions: Vec<Prompt>,
}

impl JobStepResult {
    /// Create a new JobStepResult
    pub fn new() -> Self {
        Self {
            initial_message_datetime: String::new(),
            step_revisions: Vec::new(),
        }
    }

    /// Adds a new Prompt into step_revisions, thus denoting that
    /// this is the latest edit/response.
    pub fn add_new_step_revision(&mut self, prompt: Prompt) {
        self.step_revisions.push(prompt);
    }

    /// Returns the latest revisions of the Job Step Result if one exists
    pub fn get_result_prompt(&self) -> Option<Prompt> {
        self.step_revisions.last().cloned()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
