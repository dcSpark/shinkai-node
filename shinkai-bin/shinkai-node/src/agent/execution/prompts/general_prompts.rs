use super::prompts::{JobPromptGenerator, Prompt, SubPrompt, SubPromptAssetType, SubPromptType};
use crate::{
    agent::job::JobStepResult, managers::model_capabilities_manager::ModelCapabilitiesManager,
    tools::router::ShinkaiTool,
};
use lazy_static::lazy_static;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, RetrievedNode};

impl JobPromptGenerator {
    pub fn convert_resource_into_subprompts(resource: BaseVectorResource) -> Vec<SubPrompt> {
        let mut subprompts = vec![];

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
            prompt.add_step_history(step_history, 10, 98, 4000);
        }

        prompt.add_content(
            "You are an assistant running in a system who only has access your own knowledge to answer any question the user provides. The user has asked:\n".to_string(),
            SubPromptType::System,
            99
        );
        prompt.add_content(user_message.to_string(), SubPromptType::User, 100);

        prompt
    }

    /// Prompt to be used for getting the LLM to generate a new/different search term if the LLM repeated
    pub fn retry_new_search_term_prompt(search_term: String, summary: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
        format!("Based on the following summary: \n\n{}\n\nYou need to come up with a unique and detailed search term that is different than the provided one: `{}`", summary, search_term),
        SubPromptType::System,
        100
    );
        prompt.add_ebnf(String::from(r#"# Search"#), SubPromptType::System, 100);

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
            format!("Here is the answer to your request: `{}`", invalid_markdown),
            SubPromptType::Assistant,
            100,
        );

        let mut wrong_string =
            r#"No that is wrong. I need it to be properly formatted as a markdown with the correct section names. "#
                .to_string();

        if let Some(md_def) = markdown_definitions.iter().next() {
            wrong_string += &format!(
                " You must fix the previous answer by outputting markdown that follows this schema: \n{}",
                md_def
            );
        }
        prompt.add_content(wrong_string, SubPromptType::User, 100);

        prompt.add_content(
            format!(
                r#"Remember to escape any double quotes that you include in the content. Respond only with the markdown specified format and absolutely no explanation or anything else: \n\n"#,
            ),
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

        prompt.add_content(format!("Here is content from a document:"), SubPromptType::User, 99);
        for node in nodes {
            prompt.add_content(format!("{}", node), SubPromptType::User, 98);
        }
        prompt.add_content(
            String::from(
                "Summarize the content using as many relevant keywords as possible. Aim for 3-4 sentences maximum. Respond using the follow markdown template and nothing else:",
            ),
            SubPromptType::User,
            100,
        );
        prompt.add_ebnf(
            String::from(r#"# Summary\n{{summary}}\n"#),
            SubPromptType::System,
            100,
        );

        prompt.add_content(do_not_mention_prompt.to_string(), SubPromptType::System, 99);

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
            "Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and \n separated paragraphs. Start your response with # Answer".to_string(),
            SubPromptType::System,
            98
        );

        prompt
    }
}

lazy_static! {
    pub static ref do_not_mention_prompt: String = "Do not mention needing further context, or information, or ask for more research, just directly provide as much information as you know: ".to_string();
}

lazy_static! {
    pub static ref bootstrap_plan_prompt: String = String::from(
        r#"

    You are an assistant running in a system who only has access to a series of tools and your own knowledge to accomplish any task.

 The user has asked the system:

    `{}`

Create a plan that the system will need to take in order to fulfill the user's task. Make sure to make separate steps for any sub-task where data, computation, or API access may need to happen from different sources.

Keep each step in the plan extremely concise/high level comprising of a single sentence each. Do not mention anything optional, nothing about error checking or logging or displaying data. Anything related to parsing/formatting can be merged together into a single step. Any calls to APIs, including parsing the resulting data from the API, should be considered as a single step.

Respond using the following EBNF and absolutely nothing else:
"{" "plan" ":" "[" string ("," string)* "]" "}"

"#
    );


// Output:
// {
// "plan": [
// "Retrieve the current date and time for New York.",
// "Query a weather API for New York's current weather using the obtained date and time.",
// "Parse the weather data to extract the current weather conditions."
// ]
// }



// Example ebnf of weather fetch output for testing
// weather-fetch-output ::= "{" "city" ":" text "," "weather-description" ":" text "," "tool" ": \"Weather Fetch\", "}"  text ::= [a-zA-Z0-9_]+

    pub static ref task_bootstrap_prompt: String = String::from(
        r#"
    You are an assistant running in a system who only has access to a series of tools, your own knowledge, and the current context of acquired info includes:

    ```
    Datetime ::= 4DIGIT "-" 2DIGIT "-" 2DIGIT "T" 2DIGIT ":" 2DIGIT ":" 2DIGIT
    ```

    The current task at hand is:

    `Query a weather API for New York's current weather using the obtained date and time.`

    
    If it is a task not pertaining to recent/current knowledge and you can respond respond directly without any external help, respond using the following EBNF and absolutely nothing else:

    `"{" "prepared" ":" true "}"`

    If you do not have the ability to respond correctly yourself, it is your goal is to find the final tool that will provide you with the capabilities you need. 
    Search to find tools which you can use, respond using the following EBNF and absolutely nothing else:

    "{" ("tool-search" ":" string) "}"

    Only respond with an answer if you are not using any tools. Make sure the response matches the EBNF and includes absolutely nothing else. 

    "#
    );
    pub static ref tool_selection_prompt: String = String::from(
        r#"

    You are an assistant running in a system who only has access to a series of tools, your own knowledge, and the current context of acquired info includes:

    ```
    Datetime: 2023-09-13T14:30:00
    ```

    The current task at hand is:

    `Query a weather API for New York's current weather using the obtained date and time.`

    Here are up to 10 of the most relevant tools available:
    1. Name: Weather Fetch - Description: Requests weather via an API given a city name.
    2. Name: Country Population - Description: Provides population numbers given a country name.
    3. Name: HTTP GET - Description: Issues an http get request to a specified URL. Note: Only fetch URLs from user's input or from output of other tools.

    It is your goal to select the final tool that will enable the system to accomplish the user's task. The system may end up needing to chain multiple tools to acquire all needed info/data, but the goal right now is to find the final tool.
    Select the name of the tool from the list above that fulfill this, respond using the following EBNF and absolutely nothing else:

    "{" ("tool" ":" string) "}"

    If none of the tools match explain what the issue is by responding using the following EBNF and absolutely nothing else:

    "{" ("error" ":" string) "}"


    ```json



        "#
    );
    pub static ref tool_ebnf_prompt: String = String::from(
        r#"

    You are an assistant running in a system who only has access to a series of tools, your own knowledge, and the current context of acquired info includes:

    ```
    Datetime: 2023-09-13T14:30:00
    ```

    The current task at hand is:

    `Query a weather API for New York's current weather using the obtained date and time.`

    The system has selected the following tool to be used:

    Tool Name: Weather Fetch
    Toolkit Name: weather-toolkit
    Description: Requests weather via an API given a city name.
    Tool Input EBNF: "{" "city" ":" text "," "datetime" ":" text "," "tool" ": \"Weather Fetch\"," "toolkit" ": \"weather-toolkit\" }"  text ::= [a-zA-Z0-9_]+ 

    Your goal is to decide whether for each field in the Tool Input EBNF, you have been provided all the needed data to fill it out fully.

  Respond using the follow markdown template and nothing else:   If all of the data/information to use the tool is available, respond using the following EBNF and absolutely nothing else:

    "{" ("prepared" ":" true) "}"
    
    If you need to acquire more information in order to use this tool (ex. user's personal data, related facts, info from external APIs, etc.) then you will need to search for other tools that provide you with this data by responding using the following EBNF and absolutely nothing else:

    "{" ("tool-search" ":" string) "}"

    ```json


    "#
    );
}
