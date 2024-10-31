use crate::{shinkai_message::shinkai_message_schemas::AssociatedUI, shinkai_utils::job_scope::JobScope};

use super::{inbox_name::InboxName, job_config::JobConfig, prompts::Prompt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait JobLike: Send + Sync {
    fn job_id(&self) -> &str;
    fn is_hidden(&self) -> bool;
    fn datetime_created(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn parent_llm_provider_id(&self) -> &str;
    fn scope(&self) -> &JobScope;
    fn conversation_inbox_name(&self) -> &InboxName;
    fn associated_ui(&self) -> Option<&AssociatedUI>;
    fn config(&self) -> Option<&JobConfig>;
    fn forked_jobs(&self) -> &Vec<ForkedJob>;
}

// Todo: Add a persistent_context: String
#[derive(Clone, Debug)]
pub struct Job {
    /// Based on uuid
    pub job_id: String,
    /// Marks if the job is hidden or not. Hidden jobs are not shown in the UI to avoid spamming
    pub is_hidden: bool,
    /// Format: "20230702T20533481346" or Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    pub datetime_created: String,
    /// Marks if the job is finished or not
    pub is_finished: bool,
    /// Identity of the parent agent. We just use a full identity name for simplicity
    pub parent_agent_or_llm_provider_id: String,
    /// What VectorResources the Job has access to when performing vector searches
    pub scope: JobScope,
    /// An inbox where messages to the agent from the user and messages from the agent are stored,
    /// enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    /// The job's step history (an ordered list of all prompts/outputs from LLM inferencing when processing steps)
    /// Under the hood this is a tree, but it looks like a simple Vec because we only care about the latest valid path
    /// based on the last message sent by the user
    pub step_history: Vec<JobStepResult>,
    /// An ordered list of the latest messages sent to the job which are yet to be processed
    pub unprocessed_messages: Vec<String>,
    /// A hashmap which holds a bunch of labeled values which were generated as output from the latest Job step
    /// Same as step_history. Under the hood this is a tree, but everything is automagically filtered and converted to a hashmap.
    pub execution_context: HashMap<String, String>,
    /// A link to the UI where the user can view the job e.g. Sheet UI
    pub associated_ui: Option<AssociatedUI>,
    /// The job's configuration
    pub config: Option<JobConfig>,
    /// Keep track of forked jobs
    pub forked_jobs: Vec<ForkedJob>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForkedJob {
    pub job_id: String,
    pub message_id: String,
}

impl JobLike for Job {
    fn job_id(&self) -> &str {
        &self.job_id
    }

    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn datetime_created(&self) -> &str {
        &self.datetime_created
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn parent_llm_provider_id(&self) -> &str {
        &self.parent_agent_or_llm_provider_id
    }

    fn scope(&self) -> &JobScope {
        &self.scope
    }

    fn conversation_inbox_name(&self) -> &InboxName {
        &self.conversation_inbox_name
    }

    fn associated_ui(&self) -> Option<&AssociatedUI> {
        self.associated_ui.as_ref()
    }

    fn config(&self) -> Option<&JobConfig> {
        self.config.as_ref()
    }

    fn forked_jobs(&self) -> &Vec<ForkedJob> {
        &self.forked_jobs
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

impl Default for JobStepResult {
    fn default() -> Self {
        Self::new()
    }
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
