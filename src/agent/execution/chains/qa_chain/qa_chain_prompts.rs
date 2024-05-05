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

        // Add previous step results from history
        let mut step_history_is_empty = true;
        if let Some(step_history) = job_step_history {
            step_history_is_empty = step_history.is_empty();
            // If no vec search results, return up to 10 as likely to be relevant
            if ret_nodes.is_empty() {
                prompt.add_step_history(step_history, 10, 96);
            } else {
                prompt.add_step_history(step_history, 2, 97);
            }
        }

        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Remember to only use single quotes (never double quotes) inside of strings that you respond with.".to_string(),
            SubPromptType::System,
            99
        );

        if let Some(summary) = summary_text {
            prompt.add_content(
                format!(
                    "Here is the current summary of content another assistant found to answer the user's question: `{}`",
                    summary
                ),
                SubPromptType::System,
                99
            );
        }

        // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
        if !ret_nodes.is_empty() {
            prompt.add_content(
                "Here is a list of relevant new content provided for you to potentially use while answering:"
                    .to_string(),
                SubPromptType::System,
                97,
            );
            for node in ret_nodes {
                if let Some(content) = node.format_for_prompt(3500) {
                    prompt.add_content(content, SubPromptType::System, 97);
                }
            }
        }

        prompt.add_content(format!("The user has asked: "), SubPromptType::System, 100);
        prompt.add_content(format!("{}.\n", user_message), SubPromptType::User, 100);

        prompt.add_content(
            format!("If you have enough information to directly answer the user's question:"),
            SubPromptType::System,
            100,
        );
        prompt.add_ebnf(
            String::from(r#"# Answer"#),
            SubPromptType::System,
            100,
        );

        let this_clause = if step_history_is_empty {
            "When the user talks about `it` or `this`, they are referencing the content."
        } else {
            "When the user talks about `it` or `this`, they are referencing the previous message."
        };

        // Tell the LLM about the previous search term (up to max 3 words to not confuse it) to avoid searching the same
        if let Some(mut prev_search) = prev_search_text {
            let words: Vec<&str> = prev_search.split_whitespace().collect();
            if words.len() > 3 {
                prev_search = words[..3].join(" ");
            }
            prompt.add_content(format!("If you need to acquire more information to properly answer the user, then you will need to think carefully and drastically improve/extend the existing summary with more information and think of a search query to find new content. Search for keywords more unique & detailed than `{}`:", prev_search), SubPromptType::System, 99);
        } else {
            prompt.add_content(format!("If you need to acquire more information to properly answer the user, then you will need to create a summary of the current content related to the user's question, and think of a search query to find new content:"), SubPromptType::System, 99);
        }

        prompt.add_ebnf(
            String::from(r#"# Search\n{{content}}\n\n# Summary\n{{content}}"#),
            SubPromptType::System,
            100,
        );

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
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Add previous step results from history
        let mut step_history_is_empty = true;
        if let Some(step_history) = job_step_history {
            step_history_is_empty = step_history.is_empty();
            // If no vec search results, return up to 10 as likely to be relevant
            if ret_nodes.is_empty() {
                prompt.add_step_history(step_history, 10, 96);
            } else {
                prompt.add_step_history(step_history, 2, 97);
            }
        }

        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Remember to only use single quotes (never double quotes) inside of strings that you respond with.".to_string(),
            SubPromptType::System,
            98
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
        // If this is the first iteration count, then we want to add the retrieved nodes as sub-prompts as it had no previous context
        if !ret_nodes.is_empty() && iteration_count == 1 {
            // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
            prompt.add_content(
                "Here is a list of relevant new content provided for you to potentially use while answering:"
                    .to_string(),
                SubPromptType::System,
                97,
            );
            for node in ret_nodes {
                if let Some(content) = node.format_for_prompt(3500) {
                    prompt.add_content(content, SubPromptType::System, 97);
                }
            }
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
            format!("Use the content to directly answer the user's question. {} Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and \n separated paragraphs. Format answer so that it is easily readable with newlines after each 2 sentences and bullet point lists as needed:", this_clause),
            SubPromptType::System,
            98
        );

        prompt.add_ebnf(
            String::from(r#"# Answer"#),
            SubPromptType::System,
            100,
        );

        prompt
    }
}
