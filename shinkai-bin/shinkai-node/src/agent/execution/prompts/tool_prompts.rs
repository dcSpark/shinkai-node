use super::{
    prompts::{JobPromptGenerator, Prompt, SubPromptType},
};
use crate::{
    tools::router::ShinkaiTool,
};
use std::collections::HashMap;

impl JobPromptGenerator {
    pub fn bootstrap_plan_prompt(user_message: String) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.add_content(
            "You are an assistant running in a system who only has access to a series of tools and your own knowledge to accomplish any task.\n".to_string(),
            SubPromptType::System,
            99
        );
        prompt.add_content(user_message.to_string(), SubPromptType::User, 100);
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
            "You always remember that a PDDL is formatted like this (unrelated example): ---start example---(define (problem letseat-simple)\n    (:domain letseat)\n    (:objects\n        arm - robot\n        cupcake - cupcake\n        table - location\n        plate - location\n    )\n\n    (:init\n        (on arm table)\n        (on cupcake table)\n        (arm-empty)\n        (path table plate)\n    )\n    (:goal\n        (on cupcake plate)\n    )\n)---end example---".to_string(),
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
                "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field".to_string(),
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
            "You always remember that a PDDL is formatted like this (unrelated example): --start example---(define (domain letseat)\n    (:requirements :typing)\n\n    (:types\n        location locatable - object\n        bot cupcake - locatable\n        robot - bot\n    )\n\n    (:predicates\n        (on ?obj - locatable ?loc - location)\n        (holding ?arm - locatable ?cupcake - locatable)\n        (arm-empty)\n        (path ?location1 - location ?location2 - location)\n    )\n\n    (:action pick-up\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (on ?cupcake ?loc)\n            (arm-empty)\n        )\n        :effect (and\n            (not (on ?cupcake ?loc))\n            (holding ?arm ?cupcake)\n            (not (arm-empty))\n        )\n    )\n\n    (:action drop\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (holding ?arm ?cupcake)\n        )\n        :effect (and\n            (on ?cupcake ?loc)\n            (arm-empty)\n            (not (holding ?arm ?cupcake))\n        )\n    )\n\n    (:action move\n        :parameters (?arm - bot ?from - location ?to - location)\n        :precondition (and\n            (on ?arm ?from)\n            (path ?from ?to)\n        )\n        :effect (and\n            (not (on ?arm ?from))\n            (on ?arm ?to)\n        )\n    )\n)---end example---".to_string(),
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
                "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field".to_string(),
                SubPromptType::User, 99);
        }

        prompt.add_ebnf(
            String::from(r#"'{' 'explanation' ':' string, 'answer' ':' string '}'"#),
            SubPromptType::System,
            100,
        );

        prompt
    }
}
