use serde_json::json;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use std::sync::Arc;

use crate::managers::tool_router::ToolCallFunctionResponse;

use shinkai_message_primitives::schemas::llm_message::LlmMessage;
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunkCollection;
use shinkai_message_primitives::schemas::subprompts::SubPromptType;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;

pub struct JobPromptGenerator {}

impl JobPromptGenerator {
    /// Creates a prompt for document description generation from a list of text chunks
    pub fn simple_doc_description(descriptions: Vec<String>) -> Prompt {
        let mut prompt = Prompt::new();
        
        // Add system prompt
        prompt.add_content(
            "You are a virtual assistant that can analyze and summarize documents. Please provide a brief summary of the following documents highlighting the key points:".to_string(),
            SubPromptType::System,
            100,
        );
        
        // Add document content
        let text = descriptions.join("\n\n");
        prompt.add_content(text, SubPromptType::ExtraContext, 50);
        
        // Add user prompt
        prompt.add_content(
            "Please analyze this document and provide a concise summary highlighting the key points. Focus only on the most important information.".to_string(),
            SubPromptType::User,
            90,
        );
        
        prompt
    }
    
    pub async fn prompt_with_vector_database_results(
        _db: Arc<SqliteManager>,
        custom_system_prompt: Option<String>,
        user_message: String,
        image_files: HashMap<String, String>,
        vr_nodes: ShinkaiFileChunkCollection,
        function_calls: Option<Vec<ToolCallFunctionResponse>>,
        tools: Vec<ShinkaiTool>,
        user_chat_history: Vec<LlmMessage>,
        _job_id: String,
    ) -> Prompt {
        let mut prompt = Prompt::new();

        // Add system prompt
        let system_prompt = custom_system_prompt
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses.".to_string());

        prompt.add_content(system_prompt, SubPromptType::System, 100);

        // Add chat history
        for message in user_chat_history {
            let content = message.content.clone().unwrap_or_default();
            let role = message.role.clone().unwrap_or_default();
            if role == "user" {
                prompt.add_content(content, SubPromptType::User, 90);
            } else if role == "assistant" {
                prompt.add_content(content, SubPromptType::Assistant, 90);
            }
        }

        // Add tools if any
        if !tools.is_empty() {
            for tool in tools {
                if let Ok(tool_content) = tool.json_function_call_format() {
                    prompt.add_tool(tool_content, SubPromptType::AvailableTool, 10);
                }
            }
        }

        // Parses vector nodes as individual sub-prompts.
        // We have to parse each vector node as a separate sub-prompt, to support
        // priority pruning, and also grouping like i.e. instead of having 100 tiny messages, we have a message with the chunk
        // also this enables the LLM to get the context window full - because we'll add the primary system, user, agent prompt first.

        let has_vr_nodes = !vr_nodes.is_empty();

        if has_vr_nodes {
            for chunk in vr_nodes.chunks {
                prompt.add_content(chunk.content, SubPromptType::ExtraContext, 50);
            }
        }

        // Add the user question and the preference prompt for the answer
        prompt.add_omni(user_message, image_files, SubPromptType::UserLastMessage, 95);

        // Process function calls and their responses if they exist
        if let Some(function_calls) = function_calls {
            for function_call in function_calls {
                // Convert FunctionCall to Value
                let function_call_value = json!({
                    "name": function_call.function_call.name,
                    "arguments": function_call.function_call.arguments
                });

                // We add the assistant request to the prompt
                prompt.add_function_call(function_call_value, 100);

                // We add the function response to the prompt
                prompt.add_function_call_response(serde_json::to_value(function_call).unwrap(), 100);
            }
        }
        prompt
    }
} 