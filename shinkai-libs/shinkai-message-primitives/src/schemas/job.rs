use crate::{
    shinkai_message::shinkai_message_schemas::AssociatedUI,
    shinkai_utils::job_scope::{JobScope, MinimalJobScope},
};

use super::{inbox_name::InboxName, job_config::JobConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait JobLike: Send + Sync {
    fn job_id(&self) -> &str;
    fn is_hidden(&self) -> bool;
    fn datetime_created(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn parent_llm_provider_id(&self) -> &str;
    fn scope(&self) -> &MinimalJobScope;
    fn scope_with_files(&self) -> Option<&JobScope>;
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
    /// (Simplified version) What VectorResources the Job has access to when performing vector searches
    pub scope: MinimalJobScope,
    /// (Full version) What VectorResources the Job has access to when performing vector searches, including files
    pub scope_with_files: Option<JobScope>,
    /// An inbox where messages to the agent from the user and messages from the agent are stored,
    /// enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    /// A hashmap which holds a bunch of labeled values which were generated as output from the latest Job step
    /// Under the hood this is a tree, but everything is automagically filtered and converted to a hashmap.
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

    fn scope(&self) -> &MinimalJobScope {
        &self.scope
    }

    fn scope_with_files(&self) -> Option<&JobScope> {
        self.scope_with_files.as_ref()
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
