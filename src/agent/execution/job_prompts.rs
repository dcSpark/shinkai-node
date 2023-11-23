use super::super::{error::AgentError, providers::openai::OpenAIApiMessage};
use crate::{agent::job::JobStepResult, tools::router::ShinkaiTool};
use futures::stream::ForEach;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::vector_resource_types::RetrievedNode;
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
    pub fn basic_instant_response_prompt(job_task: String, job_step_history: Option<Vec<JobStepResult>>) -> Prompt {
        let mut prompt = Prompt::new();

        // Add up to previous 10 step results from history
        if let Some(step_history) = job_step_history {
            prompt.add_step_history(step_history, 10, 98);
        }

        prompt.add_content(
            "You are an assistant running in a system who only has access your own knowledge to answer any question the user provides. The user has asked:\n".to_string(),
            SubPromptType::System,
            99
        );
        prompt.add_content(format!("{}", job_task), SubPromptType::User, 100);
        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the job_task, with the ability to further search.
    pub fn response_prompt_with_vector_search(
        job_task: String,
        ret_nodes: Vec<RetrievedNode>,
        summary_text: Option<String>,
        prev_search_text: Option<String>,
        job_step_history: Option<Vec<JobStepResult>>,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Add up to previous 10 step results from history
        if let Some(step_history) = job_step_history {
            prompt.add_step_history(step_history, 10, 98);
        }

        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible.".to_string(),
            SubPromptType::System,
            100
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
        prompt.add_content(job_task, SubPromptType::User, 100);

        prompt.add_content(
            format!("If you have enough information to directly answer the user's question:"),
            SubPromptType::System,
            100,
        );
        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

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
            String::from(r#"'{' 'search' ':' string, 'summary': 'string' }'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// A basic prompt for answering based off of vector searching content which explains to the LLM
    /// that it should use them as context to answer the job_task with no option to further search.
    pub fn response_prompt_with_vector_search_final(
        job_task: String,
        ret_nodes: Vec<RetrievedNode>,
        summary_text: Option<String>,
        job_step_history: Option<Vec<JobStepResult>>,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Add up to previous 10 step results from history
        if let Some(step_history) = job_step_history {
            prompt.add_step_history(step_history, 10, 98);
        }

        prompt.add_content(
            "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible.".to_string(),
            SubPromptType::System,
            100
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

        // TODO: Either re-introduce this or delete it after testing with more QA in practice.
        // // Parses the retrieved nodes as individual sub-prompts, to support priority pruning
        // if !ret_nodes.is_empty() {
        //     prompt.add_content(
        //         "Here is a list of relevant new content provided for you to potentially use while answering:"
        //             .to_string(),
        //         SubPromptType::System,
        //         97,
        //     );
        //     for node in ret_nodes {
        //         if let Some(content) = node.format_for_prompt(3500) {
        //             prompt.add_content(content, SubPromptType::System, 97);
        //         }
        //     }
        // }

        let pre_task_text = format!("The user has asked: ");
        prompt.add_content(pre_task_text, SubPromptType::System, 99);
        prompt.add_content(job_task, SubPromptType::User, 100);

        prompt.add_content(
            format!("Use the content to directly answer the user's question with as much information as is available. Make the answer very readable and easy to understand:"),
            SubPromptType::System,
            98
        );

        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

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
        prompt.add_ebnf(
            String::from(r#"'{' 'search' ':' string }'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// Inferences the LLM again asking it to take its previous answer and make sure it responds with a proper JSON object
    /// that we can parse, according to one of the EBNFs from the original prompt.
    pub fn basic_json_retry_response_prompt(non_json_answer: String, original_prompt: Prompt) -> Prompt {
        let mut prompt = Prompt::new();

        // Iterate through the original prompt and only keep the EBNF subprompts
        for sub_prompt in original_prompt.sub_prompts {
            if let SubPrompt::EBNF(prompt_type, ebnf, _) = sub_prompt {
                prompt.add_ebnf(ebnf, prompt_type, 99);
            }
        }

        prompt.add_content(
            format!("Here is the answer to your request: `{}`", non_json_answer),
            SubPromptType::System,
            100,
        );
        prompt.add_content(
            String::from(
                r#"No, I need it to be properly formatted as JSON. Look at the EBNF definitions you provided earlier and respond exactly the same but formatted using the best matching one. ```json"#,
            ),
            SubPromptType::User, 100
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
            prompt.add_content(format!("{}", node), SubPromptType::User, 99);
        }
        prompt.add_content(
            format!("Take a deep breath and summarize the content using as many relevant keywords as possible. Aim for 3-4 sentences maximum."),
            SubPromptType::User,
            100
        );
        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt.add_content(do_not_mention_prompt.to_string(), SubPromptType::System, 99);

        prompt
    }

    pub fn bootstrap_plan_prompt(job_task: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an assistant running in a system who only has access to a series of tools and your own knowledge to accomplish any task.\n".to_string(),
            SubPromptType::System,
            99
        );
        prompt.add_content(format!("{}", job_task), SubPromptType::User, 100);
        prompt.add_content(
            String::from(
                "Create a plan that the system will need to take in order to fulfill the user's task. Make sure to make separate steps for any sub-task where data, computation, or API access may need to happen from different sources.\n\nKeep each step in the plan extremely concise/high level comprising of a single sentence each. Do not mention anything optional, nothing about error checking or logging or displaying data. Anything related to parsing/formatting can be merged together into a single step. Any calls to APIs, including parsing the resulting data from the API, should be considered as a single step."
            ),
            SubPromptType::System,
            100
        );
        prompt.add_ebnf(
            String::from("{{'plan': ['string' (, 'string')*]}}"),
            SubPromptType::System,
            100,
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
            99
        );

        prompt.add_content(
            format!("The current task at hand is:\n\n`{}`", task),
            SubPromptType::User,
            100,
        );

        prompt.add_content(
            format!("We have selected the following tool to be used:\n\n{}", tool_summary),
            SubPromptType::System,
            100,
        );

        prompt.add_content(
            String::from(
                "Your goal is to decide whether for each field in the Tool Input EBNF, you have been provided all the needed data to fill it out fully.\nIf all of the data/information to use the tool is available,"
            ),
            SubPromptType::System,
            100
        );

        prompt.add_ebnf(String::from("{{'prepared': true}}"), SubPromptType::User, 100);

        prompt.add_content(
            String::from(

                "If you need to acquire more information in order to use this tool (ex. user's personal data, related facts, info from external APIs, etc.) then you will need to search for other tools that provide you with this data,"
            ),
            SubPromptType::System,
            100
        );

        prompt.add_ebnf(String::from("{{'tool-search': 'string'}}"), SubPromptType::User, 100);

        prompt
    }

    /// Prompt for having the LLM generate a PDDL plan given some tools
    pub fn pddl_plan_problem_generation_prompt(
        task: String,
        pddl_domain: String,
        tools: Vec<ShinkaiTool>,
        previous: Option<String>,
        previous_error: Option<String>,
    ) -> Prompt {
        let tools_summary = tools
            .iter()
            .filter_map(|tool| tool.describe_formatted_tool_summary(false).ok())
            .collect::<Vec<String>>()
            .join("\n\n");

        let mut prompt = Prompt::new();
        prompt.add_content(
            format!(
                "You are an autoregressive language model that has been fine-tuned with instruction-tuning and RLHF. You carefully provide accurate, factual, thoughtful, nuanced answers, and are brilliant at reasoning. If you think there might not be a correct answer, you say so.  Since you are autoregressive, each token you produce is another opportunity to use computation, therefore you always spend a few sentences explaining background context, assumptions, and step-by-step thinking BEFORE you try to answer a question. You are a very helpful assistant with PDDL planning expertise and access to a series of tools. The only tools at your disposal for PDDL planing are: ---tools--- {} ---end_tools---",
                tools_summary
            ),
            SubPromptType::System,
            100
        );

        prompt.add_content(
            format!(
                "You always remember that a PDDL is formatted like this (unrelated example): ---start example---(define (problem letseat-simple)\n    (:domain letseat)\n    (:objects\n        arm - robot\n        cupcake - cupcake\n        table - location\n        plate - location\n    )\n\n    (:init\n        (on arm table)\n        (on cupcake table)\n        (arm-empty)\n        (path table plate)\n    )\n    (:goal\n        (on cupcake plate)\n    )\n)---end example---"
            ),
            SubPromptType::User,
            100
        );

        // This is the PDDL (Problem): {}.
        prompt.add_content(
            format!("The current task is to: '{}'. Implement a plan using PDDL representation using the available tools. Make it simple but effective and start your response with: (define (problem ", task),
            SubPromptType::User,
            100
        );

        if previous.is_some() && previous_error.is_some() {
            prompt.add_content(
                format!(
                    "Here is the previous plan you generated: '{}' but it has an error: {}. Take a deep breath and think step by step, explain how to fix it in the explanation field and then fix it in answer field if you are able to, if you are not certain, then start all over.",
                    previous.unwrap().replace("\\n", " "),
                    previous_error.unwrap()
                ),
                SubPromptType::User,
                100
            );
        } else {
            prompt.add_content(
                format!(
                    "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field",
                ),
                SubPromptType::User, 99);
        }

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

    /// Prompt for having the LLM generate a PDDL plan given some tools
    pub fn pddl_plan_domain_generation_prompt(
        task: String,
        tools: Vec<ShinkaiTool>,
        previous: Option<String>,
        previous_error: Option<String>,
    ) -> Prompt {
        let tools_summary = tools
            .iter()
            .filter_map(|tool| tool.describe_formatted_tool_summary(true).ok())
            .collect::<Vec<String>>()
            .join("\n\n");

        let mut prompt = Prompt::new();
        prompt.add_content(
            format!(
                "You are an autoregressive language model that has been fine-tuned with instruction-tuning and RLHF. You carefully provide accurate, factual, thoughtful, nuanced answers, and are brilliant at reasoning. If you think there might not be a correct answer, you say so.  Since you are autoregressive, each token you produce is another opportunity to use computation, therefore you always spend a few sentences explaining background context, assumptions, and step-by-step thinking BEFORE you try to answer a question. You are a very helpful assistant with PDDL planning expertise and access to a series of tools. The only tools at your disposal for PDDL planing are: ---tools--- {} ---end_tools---",
                tools_summary
            ),
            SubPromptType::System,
            100
        );

        prompt.add_content(
            format!(
                "You always remember that a PDDL is formatted like this (unrelated example): --start example---(define (domain letseat)\n    (:requirements :typing)\n\n    (:types\n        location locatable - object\n        bot cupcake - locatable\n        robot - bot\n    )\n\n    (:predicates\n        (on ?obj - locatable ?loc - location)\n        (holding ?arm - locatable ?cupcake - locatable)\n        (arm-empty)\n        (path ?location1 - location ?location2 - location)\n    )\n\n    (:action pick-up\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (on ?cupcake ?loc)\n            (arm-empty)\n        )\n        :effect (and\n            (not (on ?cupcake ?loc))\n            (holding ?arm ?cupcake)\n            (not (arm-empty))\n        )\n    )\n\n    (:action drop\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (holding ?arm ?cupcake)\n        )\n        :effect (and\n            (on ?cupcake ?loc)\n            (arm-empty)\n            (not (holding ?arm ?cupcake))\n        )\n    )\n\n    (:action move\n        :parameters (?arm - bot ?from - location ?to - location)\n        :precondition (and\n            (on ?arm ?from)\n            (path ?from ?to)\n        )\n        :effect (and\n            (not (on ?arm ?from))\n            (on ?arm ?to)\n        )\n    )\n)---end example---"
            ),
            SubPromptType::User,
            99
        );

        prompt.add_content(
            format!("The current task at hand is to: '{}'. Implement a throughout plan using PDDL representation using the available tools. (define (domain ", task),
            SubPromptType::User,
            100
        );

        if previous.is_some() && previous_error.is_some() {
            prompt.add_content(
                format!(
                    "Here is the previous plan you generated: '{}' but it has an error: {}. Take a deep breath and think step by step, explain how to fix it in the explanation field and then fix it in answer field if you are able to, if you are not certain, then start all over.",
                    previous.unwrap().replace("\\n", " "),
                    previous_error.unwrap()
                ),
                SubPromptType::User,
                99
            );
        } else {
            prompt.add_content(
                format!(
                    "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field",
                ),
                SubPromptType::User, 99);
        }

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }

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

        prompt.add_ebnf(
            String::from(r#"'{' 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubPromptType {
    User,
    System,
    Assistant,
}

impl ToString for SubPromptType {
    fn to_string(&self) -> String {
        match self {
            SubPromptType::User => "user".to_string(),
            SubPromptType::System => "system".to_string(),
            SubPromptType::Assistant => "assistant".to_string(),
        }
    }
}

/// Sub-prompts are composed of a 3-element tuple of (SubPromptType, text, priority_value)
/// Priority_value is a number between 0-100, where the higher it is the less likely it will be
/// removed if LLM context window limits are reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubPrompt {
    Content(SubPromptType, String, u8),
    EBNF(SubPromptType, String, u8),
}

/// Struct that represents a prompt to be used for inferencing an LLM
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    /// Sub-prompts that make up this prompt
    pub sub_prompts: Vec<SubPrompt>,
    /// The lowest priority value held in sub_prompts
    pub lowest_priority: u8,
    /// The highest priority value held in sub_prompts
    pub highest_priority: u8,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            sub_prompts: Vec::new(),
            lowest_priority: 0,
            highest_priority: 0,
        }
    }

    pub fn to_json(&self) -> Result<String, AgentError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self, AgentError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Adds a sub-prompt that holds any String content.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_content(&mut self, content: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::Content(prompt_type, content, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds an ebnf sub-prompt.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_ebnf(&mut self, ebnf: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::EBNF(prompt_type, ebnf, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds a single sub-prompt.
    /// Updates the lowest and highest priority values of self using the
    /// existing priority value.
    pub fn add_sub_prompt(&mut self, sub_prompt: SubPrompt) {
        self.add_sub_prompts(vec![sub_prompt]);
    }

    /// Adds multiple pre-prepared sub-prompts.
    /// Updates the lowest and highest priority values of self using the
    /// existing priority values of the sub_prompts.
    pub fn add_sub_prompts(&mut self, sub_prompts: Vec<SubPrompt>) {
        for sub_prompt in sub_prompts {
            match &sub_prompt {
                SubPrompt::Content(_, _, priority) | SubPrompt::EBNF(_, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
            }
            self.sub_prompts.push(sub_prompt);
        }
    }

    /// Adds multiple pre-prepared sub-prompts with a new priority value.
    /// The new priority value will be applied to all input sub-prompts.
    pub fn add_sub_prompts_with_new_priority(&mut self, sub_prompts: Vec<SubPrompt>, new_priority: u8) {
        let capped_priority_value = std::cmp::min(new_priority, 100) as u8;
        let mut updated_sub_prompts = Vec::new();
        for mut sub_prompt in sub_prompts {
            match &mut sub_prompt {
                SubPrompt::Content(_, _, priority) => *priority = capped_priority_value,
                SubPrompt::EBNF(_, _, priority) => *priority = capped_priority_value,
            }
            updated_sub_prompts.push(sub_prompt);
        }
        self.add_sub_prompts(updated_sub_prompts);
    }

    /// Adds previous results from step history into the Prompt, up to max_previous_history amount.
    /// Of note, priority value must be between 0-100.
    pub fn add_step_history(&mut self, mut history: Vec<JobStepResult>, max_previous_history: u64, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100) as u8;
        let mut count = 0;
        let mut sub_prompts_list = Vec::new();

        // sub_prompts_list.push(SubPrompt::Content(
        //     SubPromptType::System,
        //     "Here are the previous conversation messages:".to_string(),
        //     priority_value,
        // ));

        while let Some(step) = history.pop() {
            if let Some(prompt) = step.get_result_prompt() {
                for sub_prompt in prompt.sub_prompts {
                    sub_prompts_list.push(sub_prompt);
                }
                count += 1;
                if count >= max_previous_history {
                    break;
                }
            }
        }

        self.add_sub_prompts_with_new_priority(sub_prompts_list, capped_priority_value);
    }

    /// Removes the first sub-prompt from the end of the sub_prompts list that has the lowest priority value.
    /// Used primarily for cutting down prompt when it is too large to fit in context window.
    pub fn remove_lowest_priority_sub_prompt(&mut self) -> Option<SubPrompt> {
        let lowest_priority = self.lowest_priority;
        if let Some(position) = self.sub_prompts.iter().rposition(|sub_prompt| match sub_prompt {
            SubPrompt::Content(_, _, priority) | SubPrompt::EBNF(_, _, priority) => *priority == lowest_priority,
        }) {
            return Some(self.sub_prompts.remove(position));
        }
        None
    }

    /// Validates that there is at least one EBNF sub-prompt to ensure
    /// the LLM knows what to output.
    pub fn check_ebnf_included(&self) -> Result<(), AgentError> {
        if !self
            .sub_prompts
            .iter()
            .any(|prompt| matches!(prompt, SubPrompt::EBNF(_, _, _)))
        {
            return Err(AgentError::UserPromptMissingEBNFDefinition);
        }
        Ok(())
    }

    fn generate_ebnf_response_string(&self, ebnf: &str) -> String {
        format!(
            "Respond using the following EBNF and absolutely nothing else: {} ",
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
                SubPrompt::Content(_, content, _) => content.clone(),
                SubPrompt::EBNF(_, ebnf, _) => self.generate_ebnf_response_string(ebnf),
            })
            .collect::<Vec<String>>()
            .join("\n")
            + "\n"
            + &json_response_required;
        Ok(content)
    }

    /// Generates a tuple of a list of ChatCompletionRequestMessages and their token length,
    /// ready to be used with OpenAI inferencing.
    fn generate_chat_completion_messages(
        &self,
        model: &str,
    ) -> Result<(Vec<ChatCompletionRequestMessage>, usize), AgentError> {
        let mut tiktoken_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        let mut current_length: usize = 0;

        // Process all sub-prompts in their original order
        for sub_prompt in &self.sub_prompts {
            let (prompt_type, text) = match sub_prompt {
                SubPrompt::Content(prompt_type, content, _) => (prompt_type, content.clone()),
                SubPrompt::EBNF(prompt_type, ebnf, _) => {
                    let ebnf_string = self.generate_ebnf_response_string(ebnf);
                    (prompt_type, ebnf_string)
                }
            };

            let new_message = ChatCompletionRequestMessage {
                role: prompt_type.to_string(),
                content: Some(text),
                name: None,
                function_call: None,
            };

            let new_message_tokens = num_tokens_from_messages(model, &[new_message.clone()])
                .map_err(|e| AgentError::TokenizationError(e.to_string()))?;

            tiktoken_messages.push(new_message);
            current_length += new_message_tokens;
        }

        Ok((tiktoken_messages, current_length))
    }

    /// Processes all sub-prompts into a single output String in OpenAI's message format.
    pub fn generate_openai_messages(
        &self,
        max_prompt_tokens: Option<usize>,
    ) -> Result<Vec<ChatCompletionRequestMessage>, AgentError> {
        self.check_ebnf_included()?;

        // We take about half of a default total 4097 if none is provided
        let limit = max_prompt_tokens.unwrap_or((2700 as usize).try_into().unwrap());
        let model = "gpt-4";
        let mut prompt_copy = self.clone();
        let mut tiktoken_messages = vec![];

        // Keep looping and removing low priority sub-prompts until we are below the limit
        loop {
            let (completion_messages, token_count) = prompt_copy.generate_chat_completion_messages(model)?;
            if token_count < limit {
                tiktoken_messages = completion_messages;
                break;
            } else {
                prompt_copy.remove_lowest_priority_sub_prompt();
            }
        }

        Ok(tiktoken_messages)
    }

    // First version of generic. Probably we will need to pass a model name and a max tokens
    // to this function. No any model name will work with the tokenizers so probably we will need
    // a new function to get the max tokens for a given model or a fallback (maybe just length / 3).
    /// TODO: Update to work with priority system for prompt size reducing
    pub fn generate_genericapi_messages(&self, max_prompt_tokens: Option<usize>) -> Result<String, AgentError> {
        eprintln!("generate_genericapi_messages subprompts: {:?}", self.sub_prompts);
        self.check_ebnf_included()?;

        // TODO: Update to Llama tokenizer here
        let limit = max_prompt_tokens.unwrap_or((4000 as usize).try_into().unwrap());
        // let model = "llama2"; // TODO: change to something that actually fits

        let mut messages: Vec<String> = Vec::new();
        let mut current_length: usize = 0;
        let mut user_content_added = false;
        let mut system_content_added = false;
        let mut at_least_one_user_content = false;
        let mut first_user_content: Option<String> = None;
        let mut first_user_content_position: Option<usize> = None;

        // First, calculate the total length of EBNF content. We want to add it no matter what or
        // the response will be invalid.
        for sub_prompt in &self.sub_prompts {
            if let SubPrompt::EBNF(_, ebnf, _) = sub_prompt {
                let new_message = self.generate_ebnf_response_string(ebnf);
                current_length += new_message.len();
            }
        }

        // Then, process all sub-prompts in their original order
        for (i, sub_prompt) in self.sub_prompts.iter().enumerate() {
            match sub_prompt {
                SubPrompt::Content(prompt_type, content, priority_value) => {
                    if content == &*do_not_mention_prompt || content == "" {
                        continue;
                    }
                    let mut new_message = "".to_string();
                    if prompt_type == &SubPromptType::System || prompt_type == &SubPromptType::Assistant {
                        new_message = format!("{}", content.clone());
                    } else {
                        new_message = format!("- {}", content.clone());
                        at_least_one_user_content = true;
                        if first_user_content.is_none() {
                            first_user_content = Some(new_message.clone());
                            first_user_content_position = Some(i);
                        }
                    }

                    if prompt_type == &SubPromptType::User {
                        at_least_one_user_content = true;
                    }

                    let new_message_length = new_message.len();
                    if current_length + new_message_length > limit {
                        continue;
                    }
                    messages.push(new_message);
                    current_length += new_message_length;

                    match prompt_type {
                        SubPromptType::User => user_content_added = true,
                        SubPromptType::System => system_content_added = true,
                        SubPromptType::Assistant => system_content_added = true,
                    }
                }
                SubPrompt::EBNF(_, ebnf, _) => {
                    let new_message = self.generate_ebnf_response_string(ebnf);
                    messages.push(new_message);
                }
            }
        }

        if !at_least_one_user_content {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Error,
                "No content was added to compute the prompt",
            );
        }

        if !user_content_added && first_user_content.is_some() {
            let remaining_tokens = limit - current_length;
            let truncated_content = format!("{}...", &first_user_content.unwrap()[..remaining_tokens - 3]);
            if let Some(position) = first_user_content_position {
                messages.insert(position, truncated_content.to_string());
            }
        } else if !user_content_added {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Error,
                "No user content was added to compute the prompt",
            );
        }

        if !system_content_added {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Error,
                "No system content was added to compute the prompt",
            );
        }

        let output = messages.join(" ");
        eprintln!("generate_genericapi_messages output: {:?}", output);
        Ok(output)
    }
}

lazy_static! {
    static ref do_not_mention_prompt: String = "Do not mention needing further context, or information, or ask for more research, just directly provide as much information as you know: ".to_string();
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
