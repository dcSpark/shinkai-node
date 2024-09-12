use super::{
    prompts::{JobPromptGenerator, Prompt},
    subprompts::{SubPromptAssetType, SubPromptType},
};
use lazy_static::lazy_static;

impl JobPromptGenerator {
    pub fn simple_doc_description(nodes: Vec<String>) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an advanced assistant who who is specialized in summarizing information. Do not ask for further context or information in your answer, simply summarize as much information as possible.".to_string(),
            SubPromptType::System,
            99
        );

        prompt.add_content("Here is content from a document:".to_string(), SubPromptType::User, 99);
        for node in nodes {
            prompt.add_content(node.to_string(), SubPromptType::User, 98);
        }
        prompt.add_content(
            String::from(
                "Summarize the content using as many relevant keywords as possible. Aim for 3-4 sentences maximum",
            ),
            SubPromptType::User,
            100,
        );

        prompt.add_content(DO_NOT_MENTION_PROMPT.to_string(), SubPromptType::System, 99);

        prompt
    }

    /// Prompt for having the description of a cron translated to a cron expression
    pub fn image_to_text_analysis(description: String, image: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are a very helpful assistant that's very good at completing a task.".to_string(),
            SubPromptType::System,
            100,
        );
        prompt.add_content(description, SubPromptType::User, 100);
        prompt.add_asset(
            SubPromptAssetType::Image,
            image,
            String::from("auto"),
            SubPromptType::User,
            100,
        );

        prompt
    }
}

lazy_static! {
    pub static ref DO_NOT_MENTION_PROMPT: String = "Do not mention needing further context, or information, or ask for more research, just directly provide as much information as you know: ".to_string();
}
