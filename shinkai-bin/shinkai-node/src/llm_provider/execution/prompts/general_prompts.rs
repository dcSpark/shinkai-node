use super::{prompts::{JobPromptGenerator, Prompt}, subprompts::{SubPrompt, SubPromptAssetType, SubPromptType}};
use crate::llm_provider::job::JobStepResult;
use lazy_static::lazy_static;
use shinkai_vector_resources::vector_resource::BaseVectorResource;

impl JobPromptGenerator {
    pub fn convert_resource_into_subprompts(resource: BaseVectorResource) -> Vec<SubPrompt> {
        let subprompts = vec![];

        // // If it is an ordered vector resource, then we can just add each node as a subprompt
        // if let Ok(ord_res) = resource.as_ordered_vector_resource() {
        //     let mut subprompts = Vec::new();
        //     for ret_node in ord_res.retrieve_all_nodes_ordered() {
        //         ret_nodes.push(ret_node);
        //     }
        // }
        // // If its not an ordered vector resource, just fetch the nodes in whatever order
        // else {
        //     ret_nodes.extend(resource.as_trait_object().retrieve_nodes_exhaustive_unordered(None));
        // }

        // for ret_node in ret_nodes {
        //     if let Ok() = ret_node.get_text_content() {
        //         let content = ret_node.node.content;
        //         subprompts.push(SubPrompt::Content(content));
        //     } else if let Ok() = ret_node.get_resource_content() {
        //     }
        // }

        // let mut subprompts = Vec::new();
        // subprompts.push(SubPrompt::Text(resource.content));

        subprompts
    }

    /// Temporary prompt to just get back a response from the LLM with no tools or context or anything bonus
    pub fn basic_instant_response_prompt(user_message: String, job_step_history: Option<Vec<JobStepResult>>) -> Prompt {
        let mut prompt = Prompt::new();

        // Add up to previous step results from history
        if let Some(step_history) = job_step_history {
            prompt.add_step_history(step_history, 98);
        }

        prompt.add_content(
            "You are an assistant running in a system who only has access your own knowledge to answer any question the user provides. The user has asked:\n".to_string(),
            SubPromptType::System,
            99
        );
        prompt.add_content(user_message.to_string(), SubPromptType::User, 100);

        prompt
    }

    /// Inferences the LLM again asking it to take its previous answer and make sure it responds with a markdown that has the proper key
    pub fn basic_fix_markdown_to_include_proper_key(
        invalid_markdown: String,
        original_prompt: Prompt,
        key_to_correct: String,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        /// TODO: Make the markdown subprompts unique like EBNF was
        let markdown_definitions: Vec<String> = original_prompt
            .sub_prompts
            .iter()
            .filter(|subprompt| {
                subprompt.get_content().to_lowercase().contains("md")
                    && subprompt.get_content().to_lowercase().contains(&key_to_correct)
            })
            .map(|subprompt| subprompt.get_content())
            .collect();

        prompt.add_content(
            format!("Here is your previous response: `{}`", invalid_markdown),
            SubPromptType::User,
            100,
        );

        let mut wrong_string =
            r#"It's formatted incorrectly. It needs to be properly formatted as markdown."#
                .to_string();

        if let Some(md_def) = markdown_definitions.first() {
            wrong_string += &format!(
                " You must fix the previous answer by outputting markdown that follows this schema: \n{}",
                md_def
            );
        }
        prompt.add_content(wrong_string, SubPromptType::User, 100);

        prompt.add_content(
            r#"Remember to escape any double quotes that you include in the content. Ideally respond using markdown."#.to_string(),
            SubPromptType::User,
            100,
        );

        prompt
    }

    /// Prompt optimized to generate a description based on the first pages of a document
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
        prompt.add_content(
            format!("The current main task at hand is: `{}`", description),
            SubPromptType::User,
            100,
        );
        prompt.add_asset(
            SubPromptAssetType::Image,
            image,
            String::from("auto"),
            SubPromptType::User,
            100,
        );

        prompt.add_content(
            "Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and \n separated paragraphs.".to_string(),
            SubPromptType::System,
            98
        );

        prompt
    }
}

lazy_static! {
    pub static ref DO_NOT_MENTION_PROMPT: String = "Do not mention needing further context, or information, or ask for more research, just directly provide as much information as you know: ".to_string();
}
