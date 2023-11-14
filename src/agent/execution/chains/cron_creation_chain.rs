use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::JobPromptGenerator;
use crate::agent::file_parsing::ParsingHelper;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::tools::argument::ToolArgument;
use crate::tools::router::ShinkaiTool;
use crate::tools::rust_tools::RustTool;
use async_recursion::async_recursion;
use pddl_parser::problem::Object;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

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
        name: "WebLinkExtractor".to_string(),
        description: "Extracts all hyperlinks from the provided HTML string.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new("", vec![]), // You need to provide the actual embedding vector here
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
        name: "WebCrawler".to_string(),
        description: "Fetches HTML content from the specified URL.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new("", vec![]), // You need to provide the actual embedding vector here
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
        name: "ContentSummarizer".to_string(),
        description: "Generates a concise summary of the provided text content.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new("", vec![]), // You need to provide the actual embedding vector here
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
        name: "LLMStringPreparer".to_string(),
        description: "Splits a string into segments suitable for processing by a specified LLM without exceeding a maximum token count.".to_string(),
        input_args,
        output_args,
        tool_embedding: Embedding::new("", vec![]), // You need to provide the actual embedding vector here
    };

    ShinkaiTool::Rust(rust_tool)
}

#[derive(Debug, Clone, Default)]
pub struct CronCreationChainResponse {
    pub cron_expression: String,
    pub pddl_plan: String,
}

impl JobManager {
    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_cron_creation_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        full_job: Job,
        job_task: String,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        cron_description: String,           // when
        task_description: String,           // what
        object_description: Option<String>, // where
        // how_description: Option<String, // how to proceed afterwards
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<CronCreationChainResponse, AgentError> {
        println!("start_cron_creation_chain>  message: {:?}", job_task);

        // TODO: we need the vector search for the tools
        // let query = generator.generate_embedding_default(&query_text).await.unwrap();
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
            create_llm_string_preparer_tool(),
        ];

        // TODO: convert from sequential to parallel
        // Use the default prompt if not reached final iteration count, else use final prompt
        let filled_cron_prompt = if iteration_count < max_iterations {
            // Response from the previous job step
            let previous_job_step_response = if iteration_count == 0 {
                execution_context.get("previous_step_response").cloned()
            } else {
                None
            };
            JobPromptGenerator::cron_expression_generation_prompt(cron_description.clone())
        } else {
            // TODO: improve last shot
            JobPromptGenerator::cron_expression_generation_prompt(cron_description.clone())
        };

        //  // Use the default prompt if not reached final iteration count, else use final prompt
        let filled_pddl_prompt = if iteration_count < max_iterations {
            // Response from the previous job step
            let previous_job_step_response = if iteration_count == 0 {
                execution_context.get("previous_step_response").cloned()
            } else {
                None
            };
            JobPromptGenerator::pddl_plan_generation_prompt(task_description.clone(), ret_nodes)
        } else {
            // TODO: improve last shot
            JobPromptGenerator::pddl_plan_generation_prompt(task_description.clone(), ret_nodes)
        };

        // Inference the agent's LLM with the prompt. If it has an answer, the chain
        // is finished and so just return the answer response as a cleaned String
        let response_json_cron = JobManager::inference_agent(agent.clone(), filled_cron_prompt).await?;
        let response_json_pddl = JobManager::inference_agent(agent.clone(), filled_pddl_prompt).await?;

        if let Ok(answer_str_cron) = JobManager::extract_inference_json_response(response_json_cron.clone(), "answer") {
            let cleaned_answer_cron = ParsingHelper::ending_stripper(&answer_str_cron);
            println!("Cron Chain Final Answer: {:?}", cleaned_answer_cron);

            if let Ok(answer_str_pddl) =
                JobManager::extract_inference_json_response(response_json_pddl.clone(), "answer")
            {
                let cleaned_answer_pddl = ParsingHelper::ending_stripper(&answer_str_pddl);
                println!("PDDL Chain Final Answer: {:?}", cleaned_answer_pddl);

                // Return both answers
                return Ok(CronCreationChainResponse {
                    cron_expression: cleaned_answer_cron,
                    pddl_plan: cleaned_answer_pddl,
                });
            }
        }
        // If iteration_count is > max_iterations and we still don't have an answer, return an error
        else if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached(job_task.clone()));
        }

        // Recurse with the new search/summary text and increment iteration_count
        Self::start_cron_creation_chain(
            db,
            full_job,
            job_task.to_string(),
            agent,
            execution_context,
            user_profile,
            cron_description.clone(),
            task_description.clone(),
            object_description.clone(),
            iteration_count + 1,
            max_iterations,
        )
        .await
    }
}
