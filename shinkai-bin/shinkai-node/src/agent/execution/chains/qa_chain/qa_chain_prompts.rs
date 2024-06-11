use super::super::super::prompts::prompts::{JobPromptGenerator, Prompt, SubPromptType};
use crate::agent::job::JobStepResult;
use shinkai_vector_resources::vector_resource::RetrievedNode;

impl JobPromptGenerator {
    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the user_message, with the ability to further search.
    pub fn qa_response_prompt_with_vector_search(
        user_message: String,
        ret_nodes: Vec<RetrievedNode>,
        summary_text: Option<String>,
        prev_search_text: Option<String>,
        job_step_history: Option<Vec<JobStepResult>>,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        let ret_nodes_len = ret_nodes.len();

        add_setup_prompt(&mut prompt);
        add_step_history_prompt(&mut prompt, job_step_history, ret_nodes_len);

        if let Some(summary) = summary_text {
            prompt.add_content(
                format!(
                    "Here is the current summary of content another assistant found to answer the question: `{}`",
                    summary
                ),
                SubPromptType::User,
                99,
            );
        }
        // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
        if !ret_nodes.is_empty() {
            prompt.add_content(
                "Here is some extra context to answer any future user questions: --- start --- \n"
                    .to_string(),
                SubPromptType::ExtraContext,
                97,
            );
            for node in ret_nodes {
                prompt.add_ret_node_content(node, SubPromptType::ExtraContext, 96);
            }
            prompt.add_content(
                "--- end ---"
                    .to_string(),
                SubPromptType::ExtraContext,
                97,
            );
        }

        prompt.add_content(format!("{}\n Answer the question using this markdown and the extra context provided: \n # Answer \n here goes the answer\n", user_message), SubPromptType::User, 100);

        prompt
    }

    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the user_message with no option to further search.
    pub fn qa_response_prompt_with_vector_search_final(
        user_message: String,
        ret_nodes: Vec<RetrievedNode>,
        summary_text: Option<String>,
        job_step_history: Option<Vec<JobStepResult>>,
        iteration_count: u64,
        max_characters_in_prompt: usize,
    ) -> Prompt {
        eprintln!("qa_response_prompt_with_vector_search_final> Summary text: {:?}", summary_text);
        let mut prompt = Prompt::new();
        let ret_nodes_len = ret_nodes.len();

        add_setup_prompt(&mut prompt);
        
        add_step_history_prompt(&mut prompt, job_step_history, ret_nodes_len);

        // if let Some(summary) = summary_text {
        //     prompt.add_content(
        //         format!(
        //             "Here is the current content you found earlier to answer the user's question: `{}`",
        //             summary
        //         ),
        //         SubPromptType::System,
        //         99,
        //     );
        // }
        // If this is the first iteration count, then we want to add the retrieved nodes as sub-prompts as it had no previous context
        if !ret_nodes.is_empty() && iteration_count == 1 {
            // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
            prompt.add_content(
                "Here is a list of relevant new content provided for you to potentially use while answering:"
                    .to_string(),
                SubPromptType::User,
                97,
            );
            for node in ret_nodes {
                if let Some(content) = node.format_for_prompt(3500) {
                    prompt.add_content(content, SubPromptType::User, 97);
                }
            }
        }

        let user_message_with_format = format!(
            "{} \n Answer using markdown. Following this format: \n# Answer \n {{answer}}",
            user_message
        );

        prompt.add_content(user_message_with_format, SubPromptType::User, 100);

        prompt
    }
}

/// Adds initial setup text sub-prompt for qa chain
fn add_setup_prompt(prompt: &mut Prompt) {
    prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Use the content to directly answer the user's question. If the user talks about `it` or `this`, they are referencing the previous message.\n Respond using the following markdown schema and nothing else:\n # Answer \nhere goes the answer\n".to_string(),
            SubPromptType::System,
            98
        );
}

/// Adds previous step results from history to the prompt, and returns true if the step history is empty
pub fn add_step_history_prompt(
    prompt: &mut Prompt,
    job_step_history: Option<Vec<JobStepResult>>,
    ret_nodes_len: usize,
) {
    if let Some(step_history) = job_step_history {
        // If no vec search results, return up to 10 as likely to be relevant
        if ret_nodes_len == 0 {
            prompt.add_step_history(step_history, 96); // Note: why 96 and 97?? maybe we could have Enum that translates to numbers so it's more clear
        } else {
            prompt.add_step_history(step_history, 97);
        }
    }
}
