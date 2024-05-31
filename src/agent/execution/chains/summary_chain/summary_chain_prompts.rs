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
        mut resource_sub_prompts: Vec<SubPrompt>,
        resource_source: VRSourceReference,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        add_setup_prompt(&mut prompt);

        // Add the source if available
        if resource_source.is_none() {
            prompt.add_content(String::from("Here is the content:"), SubPromptType::System, 100);
        } else {
            prompt.add_content(
                format!("Here is the content from {}: ", resource_source.format_source_string()),
                SubPromptType::System,
                100,
            );
        }

        // Add the resource sub prompts
        prompt.add_sub_prompts(resource_sub_prompts);

        let task_message = "Your task is to summarize the content by providing a relevant title, writing an introductory paragraph explaining the high-level context of the content, and at least 5 bulletpoints in a list highlighting the main topics or chapters in the content (with 1-2 sentences describing each).\n Respond using the following markdown template and nothing else (no references). Don't forget to put all content under the top-level `# Answer`:\n";
        prompt.add_content(task_message.to_string(), SubPromptType::User, 100);

        let markdown_message = r#"# Answer\n ## {{content title here}}\n\n{{introductory paragraph here}}\n - **{{bulletpoint title here}}**: {{bulletpoint description here}}\n - **{{bulletpoint title here}}**: {{bulletpoint description here}}\n - **{{bulletpoint title here}}**: {{bulletpoint description here}}\n"#;
        prompt.add_content(markdown_message.to_string(), SubPromptType::User, 100);

        let task_message = "Do not respond with absolutely anything else, except with the above markdown template, filling it in with info to fulfill the user's summary request:\n";
        prompt.add_content(task_message.to_string(), SubPromptType::System, 100);

        prompt
    }
}

/// Adds initial setup text sub-prompt for qa chain
fn add_setup_prompt(prompt: &mut Prompt) {
    prompt.add_content(
            "You are an advanced assistant who summarizes content extremely well. Do not ask for further context or information in your answer, or respond with anything but markdown.".to_string(),
            SubPromptType::System,
            98
        );
}
