use super::{
    super::super::error::AgentError,
    general_prompts::do_not_mention_prompt,
    prompts::{JobPromptGenerator, Prompt, SubPrompt, SubPromptAssetType, SubPromptType},
};
use crate::{
    agent::job::JobStepResult, managers::model_capabilities_manager::ModelCapabilitiesManager,
    tools::router::ShinkaiTool,
};
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::collections::HashMap;

impl JobPromptGenerator {
    /// Prompt for having the description of a cron translated to a cron expression
    pub fn cron_expression_generation_prompt(description: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            format!(
                "You are a very helpful assistant that's an expert in translating user requests to cron expressions.",
            ),
            SubPromptType::System,
            99,
        );

        // TODO: consider differences in timezones
        prompt.add_content(
            format!(
                "The current task at hand is create a cron expression using the following description:\n\n`{}`",
                description
            ),
            SubPromptType::User,
            100,
        );
        
        prompt
    }

    /// Prompt for having the description of a cron translated to a cron expression
    pub fn apply_to_website_prompt(description: String, web_content: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            format!("You are a very helpful assistant that's very good at completing a task.",),
            SubPromptType::System,
            99,
        );
        prompt.add_content(
            format!("The current task at hand is: `{}`", description),
            SubPromptType::User,
            100,
        );
        prompt.add_content(
            format!(
                "Implement the task previously mentioned for the following content: ---content---\n `{}` \n---end_content---",
                web_content
            ),
            SubPromptType::User,
            100
        );
        prompt.add_content(
            format!(
                "Remember to take a deep breath first and think step by step, explain how to implement the task in the explanation field and then put the result of the task in the answer field",
            ),
            SubPromptType::User,
            100);

        prompt.add_ebnf(
            String::from(r#"# Explanation"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// Prompt for having the description of a cron translated to a cron expression
    pub fn cron_web_task_requires_links(description: String, summary: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            format!("You are a very helpful assistant that's very good at completing a task.",),
            SubPromptType::System,
            100,
        );
        prompt.add_content(
            format!("The current task at hand is: `{}`", description),
            SubPromptType::User,
            100,
        );
        prompt.add_content(
                format!(
                    "We need to do know if having links would be helpful for this task. Here is the current summary of content another assistant found to answer the user's question: `{}`",
                    summary
                ),
                SubPromptType::User,
                100,
            );
        prompt.add_content(
                format!(
                    "Remember to take a deep breath first and think step by step, explain how to implement the task in the explanation field and then put the result of the task in the answer field. You can only answer true or false nothing else.",
                ),
                SubPromptType::User,
                100);

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    pub fn cron_web_task_match_links(task_description: String, summary: String, links: Vec<String>) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            format!("You are a very helpful assistant that's very good at completing a task.",),
            SubPromptType::System,
            100,
        );
        prompt.add_content(
            format!("The original task was: ---task---{}---end_task--- and the current task at hand is for you to match the answer and add links to that task accordingly ---task_response--- {} ---end_response---", task_description, summary),
            SubPromptType::User,
            100,
        );
        prompt.add_content(
                    format!(
                        "Only add a link next to a text if you are sure that the link corresponds to that piece of content otherwise don't do it. This is the list of links ---list---`{}`---end_list---",
                        links.join(", ")
                    ),
                    SubPromptType::User,
                    100,
                );
        prompt.add_content(
                    format!(
                        "Remember to take a deep breath first and think step by step, explain how to implement the task in the explanation field and then put the result of the task in the answer field",
                    ),
                    SubPromptType::User,
                    100);

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// Prompt for having the description of a cron translated to a cron expression
    pub fn cron_subtask(description: String, web_content: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            format!("You are a very helpful assistant that's very good at completing a task.",),
            SubPromptType::System,
            100,
        );
        prompt.add_content(
            format!("The current main task at hand is: `{}`", description),
            SubPromptType::User,
            100,
        );
        prompt.add_content(
                format!(
                    "This is one of the links extracted from that task. Implement your best guess of what it needs to be done based on that task previously mentioned, summarize the following content if you can't be sure of your guess: ---content---\n`{}`\n---end_content---",
                    web_content
                ),
                SubPromptType::User,
                100,
            );
        prompt.add_content(
                format!(
                    "Remember to take a deep breath first and think step by step, explain how to implement the task in the explanation field and then put the result of the task in the answer field",
                ),
                SubPromptType::User,
                100);

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }
}
