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
        max_characters_in_prompt: usize,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        let ret_nodes_len = ret_nodes.len();

        add_setup_prompt(&mut prompt);
        let _step_history_is_empty =
            add_step_history_prompt(&mut prompt, job_step_history, ret_nodes_len, max_characters_in_prompt);

        if let Some(summary) = summary_text {
            prompt.add_content(
                format!(
                    "Here is the current summary of content another assistant found to answer the question: `{}`",
                    summary
                ),
                SubPromptType::User,
                99
            );
        }
        // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
        if !ret_nodes.is_empty() {
            prompt.add_content(
                "Here is a list of relevant new content provided for you to potentially use while answering:"
                    .to_string(),
                SubPromptType::ExtraContext,
                97,
            );
            for node in ret_nodes {
                prompt.add_ret_node_content(node, SubPromptType::ExtraContext, 97);
            }
        }

        prompt.add_content(
            "If you have enough information to directly answer the question, respond using the following markdown schema and nothing else:\n # Answer \n here goes the answer\n".to_string(),
            SubPromptType::System,
            100,
        );

        // Tell the LLM about the previous search term (up to max 3 words to not confuse it) to avoid searching the same
        // let this_clause = this_clause(step_history_is_empty, ret_nodes_len);
        if let Some(mut prev_search) = prev_search_text {
            let words: Vec<&str> = prev_search.split_whitespace().collect();
            if words.len() > 3 {
                prev_search = words[..3].join(" ");
            }
            prompt.add_content(format!("If you need to acquire more information to properly answer, then you will need to think carefully and drastically improve/extend the existing summary with more information and think of a search query to find new content. Search for keywords more unique & detailed than `{}`. Use the follow markdown schema:\n", prev_search), SubPromptType::System, 99);
        } else {
            prompt.add_content("If you need to acquire more information to properly answer, then you will need to create a summary of the current content related to the question, and think of a search query to find new content. Use the following markdown schema:\n".to_string(), SubPromptType::System, 99);
        }

        prompt.add_ebnf(
            String::from(r#"# Search\n{{search_term}}\n\n# Summary\n{{summary}}\n"#),
            SubPromptType::System,
            100,
        );

        prompt.add_content(user_message, SubPromptType::User, 100);

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
        let mut prompt = Prompt::new();
        let ret_nodes_len = ret_nodes.len();

        add_setup_prompt(&mut prompt);
        let _step_history_is_empty =
            add_step_history_prompt(&mut prompt, job_step_history, ret_nodes_len, max_characters_in_prompt);

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

/// Extra text for the prompt to explain what the user is referencing
fn this_clause(step_history_is_empty: bool, ret_nodes_len: usize) -> String {
    if step_history_is_empty {
        "If the user talks about `it` or `this`, they are referencing the content.".to_string()
    } else if ret_nodes_len == 0 {
        "If the user talks about `it` or `this`, they are referencing the previous message.".to_string()
    }
    // Case where there are both previous messages, and content in job scope
    else {
        "If the user talks about `it` or `this`, they are referencing the previous message, or content related to the previous message.".to_string()
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
    max_characters_in_prompt: usize,
) -> bool {
    // Add previous step results from history
    let mut step_history_is_empty = true;
    if let Some(step_history) = job_step_history {
        step_history_is_empty = step_history.is_empty();
        // If no vec search results, return up to 10 as likely to be relevant
        if ret_nodes_len == 0 {
            prompt.add_step_history(step_history, 10, 96, max_characters_in_prompt);
        } else {
            prompt.add_step_history(step_history, 4, 97, max_characters_in_prompt);
        }
    }

    step_history_is_empty
}
