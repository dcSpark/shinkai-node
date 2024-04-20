use super::super::prompts::{JobPromptGenerator, Prompt, SubPromptType};
use crate::agent::job::JobStepResult;
use shinkai_vector_resources::vector_resource::RetrievedNode;

impl JobPromptGenerator {
    /// Prompt for creating a detailed summary of nodes from a Vector Resource
    pub fn summary_chain_detailed_summary_prompt(
        user_message: String,
        ret_nodes: Vec<RetrievedNode>,
        summary_text: Option<String>,
        job_step_history: Option<Vec<JobStepResult>>,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Add up to previous 10 step results from history
        let mut step_history_is_empty = true;
        if let Some(step_history) = job_step_history {
            step_history_is_empty = step_history.is_empty();
            prompt.add_step_history(step_history, 10, 98);
        }

        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible using paragraphs, blocks, and bulletpoint lists. Remember to only use single quotes (never double quotes) inside of strings that you respond with.".to_string(),
            SubPromptType::System,
            100
        );

        if let Some(summary) = summary_text {
            prompt.add_content(
                format!(
                    "Here is the current content you found earlier to answer the user's question: `{}`",
                    summary
                ),
                SubPromptType::User,
                99,
            );
        }

        let pre_task_text = format!("The user has asked: ");
        prompt.add_content(pre_task_text, SubPromptType::System, 99);
        prompt.add_content(user_message, SubPromptType::User, 100);

        let this_clause = if step_history_is_empty {
            "If the user talks about `it` or `this`, they are referencing the content."
        } else {
            "If the user talks about `it` or `this`, they are referencing the previous message."
        };

        prompt.add_content(
            format!("Use the content to directly answer the user's question with as much information as is available. {} Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and `\n` separated paragraphs. Do not include further JSON inside of the `answer` field, unless the user requires it based on what they asked. Format answer so that it is easily readable with newlines after each 2 sentences and bullet point lists as needed:", this_clause),
            SubPromptType::System,
            98
        );

        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }
}
