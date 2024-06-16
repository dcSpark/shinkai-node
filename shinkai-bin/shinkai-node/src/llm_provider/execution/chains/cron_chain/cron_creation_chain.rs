use crate::cron_tasks::cron_manager::CronManager;
use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::prompts::prompts::JobPromptGenerator;
use crate::llm_provider::job::Job;
use crate::llm_provider::job_manager::JobManager;
use crate::planner::shinkai_plan::ShinkaiPlan;
use crate::tools::argument::ToolArgument;
use crate::tools::router::ShinkaiTool;
use crate::tools::rust_tools::RustTool;
use async_recursion::async_recursion;
use regex::Regex;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

use shinkai_vector_resources::embeddings::Embedding;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};

/*
    We need:
    time -> cron -> check cron is valid
    task summary -> PDDL -> check that PDDL is valid
*/

pub fn create_weblink_extractor_tool() -> ShinkaiTool {
    let input_args = vec![ToolArgument {
        name: "html".to_string(),
        arg_type: "STRING".to_string(),
        description: "HTML string to extract links from.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"(.*)\"".to_string(),
    }];

    let output_args = vec![ToolArgument {
        name: "links".to_string(),
        arg_type: "STRING".to_string(),
        description: "Array of extracted hyperlinks.".to_string(),
        is_optional: false,
        wrapper_type: "array".to_string(),
        ebnf: "\\[\"(.*)\"\\]".to_string(),
    }];

    let rust_tool = RustTool {
        name: "web_link_extractor".to_string(),
        description: "Extracts all hyperlinks from the provided HTML string.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new_empty(), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

pub fn create_web_crawler_tool() -> ShinkaiTool {
    let input_args = vec![ToolArgument {
        name: "url".to_string(),
        arg_type: "STRING".to_string(),
        description: "URL of the webpage to crawl.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"http(s)?://([\\w-]+\\.)+[\\w-]+(/[\\w- ./?%&=]*)?\"".to_string(),
    }];

    let output_args = vec![ToolArgument {
        name: "htmlContent".to_string(),
        arg_type: "STRING".to_string(),
        description: "HTML content of the crawled webpage.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"(.*)\"".to_string(),
    }];

    let rust_tool = RustTool {
        name: "html_extractor".to_string(),
        description: "Fetches HTML content from the specified URL.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new_empty(), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

pub fn create_content_summarizer_tool() -> ShinkaiTool {
    let input_args = vec![
        ToolArgument {
            name: "text".to_string(),
            arg_type: "STRING".to_string(),
            description: "Text content to summarize.".to_string(),
            is_optional: false,
            wrapper_type: "none".to_string(),
            ebnf: "\"(.*)\"".to_string(),
        },
        ToolArgument {
            name: "summaryLength".to_string(),
            arg_type: "INT".to_string(),
            description: "Desired length of the summary in number of sentences.".to_string(),
            is_optional: true,
            wrapper_type: "none".to_string(),
            ebnf: "([0-9]+)".to_string(),
        },
    ];

    let output_args = vec![ToolArgument {
        name: "summary".to_string(),
        arg_type: "STRING".to_string(),
        description: "Summarized text.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"(.*)\"".to_string(),
    }];

    let rust_tool = RustTool {
        name: "content_summarizer".to_string(),
        description: "Generates a concise summary of the provided text content. It could be a website.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new_empty(), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

pub fn create_llm_string_preparer_tool() -> ShinkaiTool {
    let input_args = vec![
        ToolArgument {
            name: "text".to_string(),
            arg_type: "STRING".to_string(),
            description: "The text to be prepared for LLM processing.".to_string(),
            is_optional: false,
            wrapper_type: "none".to_string(),
            ebnf: "\"(.*)\"".to_string(),
        },
        ToolArgument {
            name: "llmModelName".to_string(),
            arg_type: "STRING".to_string(),
            description: "The name of the LLM model to be used.".to_string(),
            is_optional: false,
            wrapper_type: "none".to_string(),
            ebnf: "\"(.*)\"".to_string(),
        },
        ToolArgument {
            name: "maxTokens".to_string(),
            arg_type: "INT".to_string(),
            description: "The maximum number of tokens permissible for the LLM model (optional).".to_string(),
            is_optional: true,
            wrapper_type: "none".to_string(),
            ebnf: "([0-9]+)".to_string(),
        },
    ];

    let output_args = vec![ToolArgument {
        name: "segments".to_string(),
        arg_type: "STRING".to_string(),
        description: "Array of text segments, each conforming to the LLM's token limit.".to_string(),
        is_optional: false,
        wrapper_type: "array".to_string(),
        ebnf: "\\[\"(.*)\"\\]".to_string(),
    }];

    let rust_tool = RustTool {
        name: "LLM_string_preparer".to_string(),
        description: "Splits a string into segments suitable for processing by a specified LLM without exceeding a maximum token count.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new_empty(), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

pub fn create_llm_caller_tool() -> ShinkaiTool {
    let input_args = vec![ToolArgument {
        name: "prompt".to_string(),
        arg_type: "STRING".to_string(),
        description: "The prompt to be processed by the LLM.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"(.*)\"".to_string(),
    }];

    let output_args = vec![ToolArgument {
        name: "response".to_string(),
        arg_type: "STRING".to_string(),
        description: "The response from the LLM.".to_string(),
        is_optional: false,
        wrapper_type: "none".to_string(),
        ebnf: "\"(.*)\"".to_string(),
    }];

    let rust_tool = RustTool {
        name: "LLM_caller".to_string(),
        description: "Ask an LLM any questions (it won't know current information).".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new_empty(), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

#[derive(Debug, Clone, Default)]
pub struct CronCreationChainResponse {
    pub cron_expression: String,
    pub pddl_plan_problem: String,
    pub pddl_plan_domain: String,
}

#[derive(Debug, Clone)]
pub struct CronCreationState {
    stage: String,
    cron_expression: Option<String>,
    pddl_plan_problem: Option<String>,
    pddl_plan_domain: Option<String>,
    previous: Option<String>,
    previous_error: Option<String>,
}

impl JobManager {
    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_cron_creation_chain(
        db: Arc<ShinkaiDB>,
        full_job: Job,
        user_message: String,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        cron_description: String,           // when
        task_description: String,           // what
        object_description: Option<String>, // where
        // how_description: Option<String, // how to proceed afterwards
        iteration_count: u64,
        max_iterations: u64,
        state: Option<CronCreationState>,
    ) -> Result<CronCreationChainResponse, LLMProviderError> {
        println!("start_cron_creation_chain>  message: {:?}", user_message);

        if iteration_count > max_iterations {
            return Err(LLMProviderError::InferenceRecursionLimitReached(user_message.clone()));
        }

        // TODO: we need the vector search for the tools
        // let query = generator
        //     .generate_embedding_default(&query_text)
        //     .await?;
        // let ret_nodes = JobManager::job_scope_vector_search(
        //     db.clone(),
        //     full_job.scope(),
        //     query,
        //     20,
        //     &user_profile.clone().unwrap(),
        //     true,
        // )
        // .await?;
        // we are hard-coding them for the time being
        let ret_nodes: Vec<ShinkaiTool> = vec![
            create_weblink_extractor_tool(),
            create_web_crawler_tool(),
            create_content_summarizer_tool(),
            create_llm_caller_tool(),
        ];

        let (filled_prompt, _response_key, next_stage) = match state.as_ref().map(|s| s.stage.as_str()) {
            None | Some("cron") => {
                let filled_cron_prompt =
                    JobPromptGenerator::cron_expression_generation_prompt(cron_description.clone());
                (filled_cron_prompt, "cron_expression", "pddl_domain")
            }
            Some("pddl_problem") => {
                let filled_pddl_problem_prompt = JobPromptGenerator::pddl_plan_problem_generation_prompt(
                    task_description.clone(),
                    state.as_ref().unwrap().pddl_plan_domain.clone().unwrap(),
                    ret_nodes.clone(),
                    state.as_ref().and_then(|s| s.previous.clone()),
                    state.as_ref().and_then(|s| s.previous_error.clone()),
                );
                (filled_pddl_problem_prompt, "pddl_plan_problem", "")
            }
            Some("pddl_domain") => {
                let filled_pddl_domain_prompt = JobPromptGenerator::pddl_plan_domain_generation_prompt(
                    task_description.clone(),
                    ret_nodes,
                    state.as_ref().and_then(|s| s.previous.clone()),
                    state.as_ref().and_then(|s| s.previous_error.clone()),
                );
                (filled_pddl_domain_prompt, "pddl_plan_domain", "pddl_problem")
            }
            _ => {
                return Err(LLMProviderError::InvalidCronCreationChainStage(
                    state.as_ref().unwrap().stage.clone(),
                ))
            }
        };

        let response_json = JobManager::inference_agent_markdown(agent.clone(), filled_prompt).await?;
        let mut cleaned_answer = String::new();

        if let Ok(answer_str) = JobManager::direct_extract_key_inference_response(response_json.clone(), "answer") {
            cleaned_answer = answer_str;
            let re = Regex::new(r"(\\+n)").unwrap();
            cleaned_answer = re.replace_all(&cleaned_answer, "").to_string();
            shinkai_log(
                ShinkaiLogOption::CronExecution,
                ShinkaiLogLevel::Debug,
                format!("Chain Final Answer: {:?}", cleaned_answer).as_str(),
            );

            let is_valid = match state.as_ref().map(|s| s.stage.as_str()) {
                None | Some("cron") => CronManager::is_valid_cron_expression(&cleaned_answer),
                Some("pddl_problem") => ShinkaiPlan::validate_pddl_problem(cleaned_answer.clone()).is_ok(),
                Some("pddl_domain") => ShinkaiPlan::validate_pddl_domain(cleaned_answer.clone()).is_ok(),
                _ => false,
            };
            shinkai_log(
                ShinkaiLogOption::CronExecution,
                ShinkaiLogLevel::Info,
                &format!("is_valid: {:?}", is_valid),
            );

            if is_valid {
                let mut new_state = state.unwrap_or_else(|| CronCreationState {
                    stage: "cron".to_string(),
                    cron_expression: None,
                    pddl_plan_problem: None,
                    pddl_plan_domain: None,
                    previous: None,
                    previous_error: None,
                });
                new_state.stage = next_stage.to_string();
                new_state.previous = None;
                new_state.previous_error = None;
                match new_state.stage.as_str() {
                    "pddl_domain" => new_state.cron_expression = Some(cleaned_answer.clone()),
                    "pddl_problem" => new_state.pddl_plan_domain = Some(cleaned_answer.clone()),
                    _ => (),
                };

                if new_state.stage.is_empty() {
                    return Ok(CronCreationChainResponse {
                        cron_expression: new_state.cron_expression.unwrap(),
                        pddl_plan_problem: cleaned_answer,
                        pddl_plan_domain: new_state.pddl_plan_domain.unwrap(),
                    });
                } else {
                    Self::start_cron_creation_chain(
                        db,
                        full_job,
                        user_message.to_string(),
                        agent,
                        execution_context,
                        user_profile,
                        cron_description.clone(),
                        task_description.clone(),
                        object_description.clone(),
                        iteration_count + 1,
                        max_iterations,
                        Some(new_state),
                    )
                    .await
                }
            } else {
                let mut new_state = state.clone().unwrap_or_else(|| CronCreationState {
                    stage: "cron".to_string(),
                    cron_expression: None,
                    pddl_plan_problem: None,
                    pddl_plan_domain: None,
                    previous: None,
                    previous_error: None,
                });
                new_state.previous = Some(cleaned_answer.clone());
                new_state.previous_error = match state.as_ref().map(|s| s.stage.as_str()) {
                    Some("pddl_domain") => ShinkaiPlan::validate_pddl_domain(cleaned_answer.clone()).err(),
                    Some("pddl_problem") => ShinkaiPlan::validate_pddl_problem(cleaned_answer.clone()).err(),
                    _ => None,
                };

                Self::start_cron_creation_chain(
                    db,
                    full_job,
                    user_message.to_string(),
                    agent,
                    execution_context,
                    user_profile,
                    cron_description.clone(),
                    task_description.clone(),
                    object_description.clone(),
                    iteration_count + 1,
                    max_iterations,
                    Some(new_state),
                )
                .await
            }
        } else {
            let mut new_state = state.clone().unwrap_or_else(|| CronCreationState {
                stage: "cron".to_string(),
                cron_expression: None,
                pddl_plan_problem: None,
                pddl_plan_domain: None,
                previous: None,
                previous_error: None,
            });
            new_state.previous = Some(cleaned_answer.clone());
            new_state.previous_error = match state.as_ref().map(|s| s.stage.as_str()) {
                Some("pddl_domain") => ShinkaiPlan::validate_pddl_domain(cleaned_answer.clone()).err(),
                Some("pddl_problem") => ShinkaiPlan::validate_pddl_problem(cleaned_answer.clone()).err(),
                _ => None,
            };
            Self::start_cron_creation_chain(
                db,
                full_job,
                user_message.to_string(),
                agent,
                execution_context,
                user_profile,
                cron_description.clone(),
                task_description.clone(),
                object_description.clone(),
                iteration_count + 1,
                max_iterations,
                Some(new_state),
            )
            .await
        }
    }
}
