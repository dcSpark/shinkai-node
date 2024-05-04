use super::super::super::prompts::prompts::{JobPromptGenerator, Prompt, SubPromptType};
use crate::agent::{
    execution::{prompts::prompts::SubPrompt, user_message_parser::ParsedUserMessage},
    job::JobStepResult,
};
use shinkai_vector_resources::{
    source::VRSourceReference,
    vector_resource::{BaseVectorResource, RetrievedNode},
};

impl JobPromptGenerator {
    /// Prompt for creating a detailed summary of nodes from a Vector Resource
    pub fn summary_chain_detailed_summary_prompt(
        user_message: ParsedUserMessage,
        resource_sub_prompts: Vec<SubPrompt>,
        resource_source: VRSourceReference,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Intro
        prompt.add_content(
                "You are an advanced assistant who summarizes content extremely well. Do not ask for further context or information in your answer, or respond with anything but markdown.".to_string(),
                SubPromptType::System,
                100
            );

        // Add the source if available
        if resource_source.is_none() {
            prompt.add_content(format!("Here is the content:",), SubPromptType::System, 100);
        } else {
            prompt.add_content(
                format!("Here is the content from {}: ", resource_source.format_source_string()),
                SubPromptType::System,
                100,
            );
        }

        // Add the resource sub prompts
        prompt.add_sub_prompts(resource_sub_prompts);

        let task_message = "Your task is to summarize the content by providing a relevant title, writing an introductory paragraph explaining the high-level context of the content, and at least 5 bulletpoints in a list highlighting the main topics and/or chapters in the content with 1-2 sentences describing each . Follow this json (holding a single markdown string) when responding, and include nothing else but the output markdown answer inside of minified json: ";
        prompt.add_content(task_message.to_string(), SubPromptType::User, 100);

        let markdown_message = r#"```\n{ "answer": " ## {{Content Title}}\n\n{{Introductory paragraph}}\n - **{{Bulletpoint Title}}**: {{Bulletpoint Description}}\n - **{{Bulletpoint Title}}**: {{Bulletpoint Description}}\n - **{{Bulletpoint Title}}**: {{Bulletpoint Description}}"}\n```"#;
        prompt.add_content(markdown_message.to_string(), SubPromptType::User, 100);

        let task_message = "Do not respond with absolutely anything else, except with the output json holding a single markdown string, which fulfills the users summary request: \n```json";
        prompt.add_content(task_message.to_string(), SubPromptType::User, 100);

        prompt
    }

    /// Prompt for creating a detailed summary of nodes from a Vector Resource
    pub fn summary_chain_detailed_summary_prompt_xml(
        user_message: ParsedUserMessage,
        resource_sub_prompts: Vec<SubPrompt>,
        resource_source: VRSourceReference,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Intro
        prompt.add_content(
                "You are an advanced assistant who summarizes content extremely well. Do not ask for further context or information in your answer, or respond with anything but markdown.".to_string(),
                SubPromptType::System,
                100
            );

        // Add the source if available
        if resource_source.is_none() {
            prompt.add_content(format!("Here is the content:",), SubPromptType::System, 100);
        } else {
            prompt.add_content(
                format!("Here is the content from {}: ", resource_source.format_source_string()),
                SubPromptType::System,
                100,
            );
        }

        // Add the resource sub prompts
        prompt.add_sub_prompts(resource_sub_prompts);

        let task_message = "Your task is to summarize the content by providing a relevant title, writing an introductory paragraph explaining the high-level context of the content, and at least 5 bulletpoints in a list highlighting the main topics and/or chapters in the content with 1-2 sentences describing each . Follow this xml (holding a single markdown string) when responding, and include nothing else but the output markdown answer: ";
        prompt.add_content(task_message.to_string(), SubPromptType::User, 100);

        let markdown_message = r#"
        <answer>
        ## [Content Title]

        [Introductory paragraph]

        - **[Bulletpoint Title]**: [Bulletpoint Description]
        - **[Bulletpoint Title]**: [Bulletpoint Description]
        - **[Bulletpoint Title]**: [Bulletpoint Description]
    
        </answer>
        "#;
        prompt.add_content(markdown_message.to_string(), SubPromptType::User, 100);

        let task_message = "Do not respond with absolutely anything else, except with the output xml holding a single markdown string, which fulfills the users summary request: \n```xml";
        prompt.add_content(task_message.to_string(), SubPromptType::User, 100);

        prompt
    }
}
