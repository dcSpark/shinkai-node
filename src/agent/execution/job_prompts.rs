use super::super::{error::AgentError, providers::openai::OpenAIApiMessage};
use crate::tools::router::ShinkaiTool;
use lazy_static::lazy_static;
use serde_json::to_string;
use shinkai_vector_resources::vector_resource_types::RetrievedDataChunk;
use std::{collections::HashMap, convert::TryInto};
use tiktoken_rs::{get_chat_completion_max_tokens, num_tokens_from_messages, ChatCompletionRequestMessage};

//
// Core Job Step Flow
//
// Note this will all happen within a single Job step. We will probably end up summarizing the context/results from previous steps into the step history to be included as the base initial context for new steps.
//
// 0. User submits an initial message/request to their AI Agent.
// 1. An initial bootstrap plan is created based on the initial request from the user.
//
// 2. We enter into "analysis phase".
// 3a. Iterating starting from the first point in the plan, we ask the LLM true/false if it can provide an answer given it's personal knowledge + current context.
// 3b. If it can then we mark this analysis step as "prepared" and go back to 3a for the next bootstrap plan task.
// 3c. If not we tell the LLM to search for tools that would work for this task.
// 4a. We return a list of tools to it, and ask it to either select one, or return an error message.
// 4b. If it returns an error message, it means the plan can not be completed/Agent has failed, and we exit/send message back to user with the error message (15).
// 4c. If it chooses one, we fetch the tool info including the input EBNF.
// 5a. We now show the input EBNF to the LLM, and ask it whether or not it has all the needed knowledge + potential data in the current context to be able to use the tool. (In either case  after the LLM chooses)
// 5b. The LLM says it has all the needed info, then we add the tool's name/input EBNF to the current context, and either go back to 3a for the next bootstrap plan task if the task is now finished/prepared, or go to 6 if this tool was searched for to find an input for another tool.
// 5c. The LLM doesn't have all the info it needs, so it performs another tool search and we go back to 4a.
// 6. After resolving 4-5 for the new tool search, the new tool's input EBNF has been added into the context window, which will allow us to go back to 5a for the original tool, which enables the LLM to now state it has all the info it needs (marking the analysis step as prepared), thus going back to 3a for the next top level task.
// 7. After iterating through all the bootstrap plan tasks and analyzing them, we have created an "execution plan" that specifies all tool calls which will need to be made.
//
// 8. We now move to the "execution phase".
// 9. Using the execution plan, we move forward alternating between inferencing the LLM and executing a tool, as dictated by the plan.
// 10. To start we inference the LLM with the first step in the plan + the input EBNF of the first tool, and tell the LLM to fill out the input EBNF with real data.
// 11. The input JSON is taken and the tool is called/executed, with the results being added into the context.
// 12. With the tool executed, we now inference the LLM with just the context + the input EBNF of the next tool that it needs to fill out (we can skip user's request text).
// 13. We iterate through the entire execution plan (looping back/forth between 11/12) and arrive at the end with a context filled with all relevant data needed to answer the user's initial request.
// 14. We inference the LLM one last time, providing it just the context + list of executed tools, and telling it to respond to the user's request by using/summarizing the results.
// 15. We add a Shinkai message into the job's inbox with the LLM's response, allowing the user to see the result.
//
//
//
//

pub struct JobPromptGenerator {}

impl JobPromptGenerator {
    /// Parses an execution context hashmap to string to be added into a content subprompt
    fn parse_context_to_string(context: HashMap<String, String>) -> String {
        context
            .into_iter()
            .map(|(key, value)| format!("{}: {}", key, value))
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Temporary prompt to just get back a response from the LLM with no tools or context or anything bonus
    pub fn basic_instant_response_prompt(job_task: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an assistant running in a system who only has access your own knowledge to answer any question the user provides. The user has asked:\n".to_string(),
            SubPromptType::System,
        );
        prompt.add_content(format!("{}", job_task), SubPromptType::User);
        prompt.add_ebnf(String::from(r#""{" "answer" ":" string "}""#), SubPromptType::System);

        prompt
    }

    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the job_task, with the ability to further search.
    pub fn response_prompt_with_vector_search(
        job_task: String,
        ret_data_chunks: Vec<RetrievedDataChunk>,
        summary_text: Option<String>,
        prev_search_text: Option<String>,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible.".to_string(),
            SubPromptType::System,
        );

        if let Some(summary) = summary_text {
            prompt.add_content(format!("Here is the current summary from another assitant of content they found to answer the user's question: {}", summary), SubPromptType::System);
        }

        // Parses the retrieved data chunks into a single string to add to the prompt
        let ret_chunks_content = RetrievedDataChunk::format_ret_chunks_for_prompt(ret_data_chunks, 3500);
        let search_context = format!(
            "Here is a list of relevant new content the user provided for you to use while answering: ``` {}```.\n",
            ret_chunks_content,
        );
        prompt.add_content(search_context, SubPromptType::User);

        prompt.add_content(format!("The user has asked: "), SubPromptType::System);
        prompt.add_content(job_task, SubPromptType::User);

        prompt.add_content(
            format!("If you have enough information to directly answer the user's question:"),
            SubPromptType::System,
        );
        prompt.add_ebnf(String::from(r#""{" "answer" ":" string "}""#), SubPromptType::System);

        // Tell the LLM about the previous search term (up to max 3 words to not confuse it) to avoid searching the same
        if let Some(mut prev_search) = prev_search_text {
            let words: Vec<&str> = prev_search.split_whitespace().collect();
            if words.len() > 3 {
                prev_search = words[..3].join(" ");
            }
            prompt.add_content(format!("If you need to acquire more information to properly answer the user, then you will need to think carefully and drastically improve/extend the existing summary with more information and think of a search query to find new content. Search for keywords more unique & detailed than `{}`:", prev_search), SubPromptType::System);
        } else {
            prompt.add_content(format!("If you need to acquire more information to properly answer the user, then you will need to create a summary of the current content, and think of a search query to find new content:"), SubPromptType::System);
        }

        prompt.add_ebnf(
            String::from(r#""{" "search" ":" string, "summary": "string" }""#),
            SubPromptType::System,
        );

        prompt
    }

    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the job_task with no option to further search.
    pub fn response_prompt_with_vector_search_final(
        job_task: String,
        ret_data_chunks: Vec<RetrievedDataChunk>,
        summary_text: Option<String>,
    ) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible.".to_string(),
            SubPromptType::System,
        );

        if let Some(summary) = summary_text {
            prompt.add_content(format!("Here is the current summary from another assitant of content they found to answer the user's question: {}", summary), SubPromptType::System);
        }

        // Parses the retrieved data chunks into a single string to add to the prompt
        let ret_chunks_content = RetrievedDataChunk::format_ret_chunks_for_prompt(ret_data_chunks, 2000);
        let search_context = format!(
            "Here is a list of relevant content the user provided for you to use while answering: ``` {}```.\n",
            ret_chunks_content,
        );
        prompt.add_content(search_context, SubPromptType::System);

        let pre_task_text = format!("The user has asked: ");
        prompt.add_content(pre_task_text, SubPromptType::System);
        prompt.add_content(job_task, SubPromptType::User);

        prompt.add_content(
            format!("Use the content to add any final details to the summary and directly answer the user's question."),
            SubPromptType::System,
        );

        prompt.add_ebnf(String::from(r#""{" "answer" ":" string "}""#), SubPromptType::System);

        prompt.add_content(
            format!("Do not mention needing further context, or information, or ask for more research, just directly provide as much information as you know:"),
            SubPromptType::System,
        );
        prompt
    }

    /// Prompt to be used for getting the LLM to generate a new/different search term if the LLM repeated
    pub fn retry_new_search_term_prompt(search_term: String, summary: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
        format!("Based on the following summary: \n\n{}\n\nYou need to come up with a unique and detailed search term that is different than the provided one: `{}`", summary, search_term),
        SubPromptType::System,
    );
        prompt.add_ebnf(String::from(r#""{" "search" ":" string }""#), SubPromptType::System);

        prompt
    }

    /// Temporary prompt to just get back a response from the LLM with no tools or context or anything bonus
    pub fn basic_json_retry_response_prompt(non_json_answer: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are a very helpful assistant running in a system who only has access your own knowledge to answer any question the user provides. You need to return the content of the following message correctly formatted as json:\n".to_string(),
            SubPromptType::System,
        );
        prompt.add_content(format!("{}", non_json_answer), SubPromptType::User);
        prompt.add_ebnf(String::from(r#""{" "answer" ":" string "}""#), SubPromptType::System);

        prompt
    }

    /// Prompt optimized to generate a description based on the first pages of a document
    pub fn simple_doc_description(chunks: Vec<String>) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
                r#"You are an autoregressive language model that has been fine-tuned with instruction-tuning and RLHF. You carefully provide accurate, factual, thoughtful, nuanced answers, and are brilliant at reasoning. If you think there might not be a correct answer, you say so. Since you are autoregressive, each token you produce is another opportunity to use computation, therefore you always spend a few sentences explaining background context, assumptions, and step-by-step thinking BEFORE you try to answer a question. Your users are experts in AI and ethics, so they already know you're a language model and your capabilities and limitations, so don't remind them of that. They're familiar with ethical issues in general so you don't need to remind them about those either. Don't be verbose in your answers, but do provide details and examples where it might help the explanation."#.to_string(),
                SubPromptType::System,
            );

        prompt.add_content(format!("Here are segements from a document:"), SubPromptType::User);
        for chunk in chunks {
            prompt.add_content(format!("{}", chunk), SubPromptType::User);
        }
        prompt.add_content(
            format!("Take a deep breath and thoroughly summarize the previous segments, making it as explanatory as possible."),
            SubPromptType::User,
        );
        prompt.add_ebnf(String::from(r#""{" "answer" ":" string "}""#), SubPromptType::System);

        prompt
    }

    pub fn bootstrap_plan_prompt(job_task: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an assistant running in a system who only has access to a series of tools and your own knowledge to accomplish any task.\n".to_string(),
            SubPromptType::System,
        );
        prompt.add_content(format!("{}", job_task), SubPromptType::User);
        prompt.add_content(
            String::from(
                "Create a plan that the system will need to take in order to fulfill the user's task. Make sure to make separate steps for any sub-task where data, computation, or API access may need to happen from different sources.\n\nKeep each step in the plan extremely concise/high level comprising of a single sentence each. Do not mention anything optional, nothing about error checking or logging or displaying data. Anything related to parsing/formatting can be merged together into a single step. Any calls to APIs, including parsing the resulting data from the API, should be considered as a single step."
            ),
            SubPromptType::System,
        );
        prompt.add_ebnf(
            String::from("{{\"plan\": [\"string\" (, \"string\")*]}}"),
            SubPromptType::System,
        );

        prompt
    }

    /// Prompt for having the LLM validate whether inputs for a given tool are available
    pub fn tool_inputs_validation_prompt(context: HashMap<String, String>, task: String, tool: ShinkaiTool) -> Prompt {
        let context_string = Self::parse_context_to_string(context);
        let tool_summary = tool.formatted_tool_summary(true); // true to include EBNF output

        let mut prompt = Prompt::new();
        prompt.add_content(
            format!(
                "You are an assistant running in a system who only has access to a series of tools, your own knowledge. The current context of acquired info includes:\n\n```\n{}\n```\n",
                context_string
            ),
            SubPromptType::System,
        );

        prompt.add_content(
            format!("The current task at hand is:\n\n`{}`", task),
            SubPromptType::User,
        );

        prompt.add_content(
            format!("We have selected the following tool to be used:\n\n{}", tool_summary),
            SubPromptType::System,
        );

        prompt.add_content(
            String::from(
                "Your goal is to decide whether for each field in the Tool Input EBNF, you have been provided all the needed data to fill it out fully.\nIf all of the data/information to use the tool is available,"
            ),
            SubPromptType::System,
        );

        prompt.add_ebnf(String::from("{{\"prepared\": true}}"), SubPromptType::User);

        prompt.add_content(
            String::from(

                "If you need to acquire more information in order to use this tool (ex. user's personal data, related facts, info from external APIs, etc.) then you will need to search for other tools that provide you with this data,"
            ),
            SubPromptType::System,
        );

        prompt.add_ebnf(String::from("{{\"tool-search\": \"string\"}}"), SubPromptType::User);

        prompt
    }
}

pub enum SubPromptType {
    User,
    System,
}

pub enum SubPrompt {
    Content(SubPromptType, String),
    EBNF(SubPromptType, String),
}

pub struct Prompt {
    pub sub_prompts: Vec<SubPrompt>,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            sub_prompts: Vec::new(),
        }
    }

    /// Adds a content sub-prompt
    pub fn add_content(&mut self, content: String, prompt_type: SubPromptType) {
        self.sub_prompts.push(SubPrompt::Content(prompt_type, content));
    }

    /// Adds an ebnf sub-prompt, which when rendered will include a prefixed
    /// string that specifies the output must match this EBNF string.
    pub fn add_ebnf(&mut self, ebnf: String, prompt_type: SubPromptType) {
        self.sub_prompts.push(SubPrompt::EBNF(prompt_type, ebnf));
    }

    /// Validates that there is at least one EBNF sub-prompt to ensure
    /// the LLM knows what to output.
    pub fn check_ebnf_included(&self) -> Result<(), AgentError> {
        if !self
            .sub_prompts
            .iter()
            .any(|prompt| matches!(prompt, SubPrompt::EBNF(_, _)))
        {
            return Err(AgentError::UserPromptMissingEBNFDefinition);
        }
        Ok(())
    }

    fn generate_ebnf_response_string(&self, ebnf: &str) -> String {
        format!(
            "```Respond using the following EBNF and absolutely nothing else:\n{}\n```",
            ebnf
        )
    }

    /// Processes all sub-prompts into a single output String.
    pub fn generate_single_output_string(&self) -> Result<String, AgentError> {
        self.check_ebnf_included()?;

        let json_response_required = String::from("```json");
        let content = self
            .sub_prompts
            .iter()
            .map(|sub_prompt| match sub_prompt {
                SubPrompt::Content(_, content) => content.clone(),
                SubPrompt::EBNF(_, ebnf) => self.generate_ebnf_response_string(ebnf),
            })
            .collect::<Vec<String>>()
            .join("\n")
            + "\n"
            + &json_response_required;
        Ok(content)
    }

    /// Processes all sub-prompts into a single output String in OpenAI's message format.
    pub fn generate_openai_messages(
        &self,
        max_prompt_tokens: Option<usize>,
    ) -> Result<Vec<ChatCompletionRequestMessage>, AgentError> {
        self.check_ebnf_included()?;

        // We assume 2048 tokens max for the prompt which is about half of the total 4097
        let limit = max_prompt_tokens.unwrap_or((2500 as usize).try_into().unwrap());
        let model = "gpt-4";

        let mut tiktoken_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        let mut current_length: usize = 0;

        // Process all sub-prompts in their original order
        for sub_prompt in &self.sub_prompts {
            let (prompt_type, text) = match sub_prompt {
                SubPrompt::Content(prompt_type, content) => (prompt_type, content.clone()),
                SubPrompt::EBNF(prompt_type, ebnf) => {
                    let ebnf_string = self.generate_ebnf_response_string(ebnf);
                    (prompt_type, ebnf_string)
                }
            };

            let role = match prompt_type {
                SubPromptType::User => "user".to_string(),
                SubPromptType::System => "system".to_string(),
            };

            let new_message = ChatCompletionRequestMessage {
                role: role.clone(),
                content: Some(text),
                name: None,
                function_call: None,
            };

            let new_message_tokens = num_tokens_from_messages(model, &[new_message.clone()])
                .map_err(|e| AgentError::TokenizationError(e.to_string()))?;
            if current_length + new_message_tokens > limit {
                break;
            }

            tiktoken_messages.push(new_message);
            current_length += new_message_tokens;
        }

        Ok(tiktoken_messages)
    }
}

lazy_static! {
    static ref bootstrap_plan_prompt: String = String::from(
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

    static ref task_bootstrap_prompt: String = String::from(
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

    ```json
    "#
    );
    static ref tool_selection_prompt: String = String::from(
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
    static ref tool_ebnf_prompt: String = String::from(
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

    If all of the data/information to use the tool is available, respond using the following EBNF and absolutely nothing else:

    "{" ("prepared" ":" true) "}"
    
    If you need to acquire more information in order to use this tool (ex. user's personal data, related facts, info from external APIs, etc.) then you will need to search for other tools that provide you with this data by responding using the following EBNF and absolutely nothing else:

    "{" ("tool-search" ":" string) "}"

    ```json


    "#
    );
}
