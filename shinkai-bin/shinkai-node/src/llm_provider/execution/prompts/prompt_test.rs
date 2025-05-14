#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::{
        llm_message::{DetailedFunctionCall, FunctionDetails, FunctionParameters, LlmMessage},
        prompts::Prompt,
        subprompts::{SubPrompt, SubPromptType},
    };
    use shinkai_tools_primitives::tools::{
        parameters::Parameters, rust_tools::RustTool, shinkai_tool::ShinkaiTool, tool_output_arg::ToolOutputArg,
    };

    use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;

    #[test]
    fn test_generate_llm_messages() {
        let concat_strings_desc = "Concatenates 2 to 4 strings.".to_string();
        let tool = RustTool::new(
            "concat_strings".to_string(),
            concat_strings_desc.clone(),
            {
                let mut params = Parameters::new();
                params.add_property(
                    "first_string".to_string(),
                    "string".to_string(),
                    "The first string to concatenate".to_string(),
                    true,
                    None,
                );
                params.add_property(
                    "second_string".to_string(),
                    "string".to_string(),
                    "The second string to concatenate".to_string(),
                    true,
                    None,
                );
                params.add_property(
                    "third_string".to_string(),
                    "string".to_string(),
                    "The third string to concatenate (optional)".to_string(),
                    false,
                    None,
                );
                params.add_property(
                    "fourth_string".to_string(),
                    "string".to_string(),
                    "The fourth string to concatenate (optional)".to_string(),
                    false,
                    None,
                );
                params
            },
            ToolOutputArg::empty(),
            None,
            "local:::__official_shinkai:::concat_strings".to_string(),
        );
        let shinkai_tool = ShinkaiTool::Rust(tool, true);

        let sub_prompts = vec![
            SubPrompt::Content(SubPromptType::System, "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Use the content to directly answer the user's question. If the user talks about `it` or `this`, they are referencing the previous message.\n Respond using the following markdown schema and nothing else:\n # Answer \nhere goes the answer\n".to_string(), 98),
            SubPrompt::Content(SubPromptType::User, "summarize this".to_string(), 97),
            SubPrompt::Content(SubPromptType::Assistant, "## What are the benefits of using Vector Resources ...\n\n".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "Here is a list of relevant new content provided for you to potentially use while answering:".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "- FAQ Shinkai Overview What's Shinkai? (Summary)  (Source: Shinkai - Ask Me Anything.docx, Section: ) 2024-05-05T00:33:00".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "- Shinkai is a comprehensive super app designed to enhance how users interact with AI. It allows users to run AI locally, facilitating direct conversations with documents and managing files converted into AI embeddings for advanced semantic searches across user data. This local execution ensures privacy and efficiency, putting control directly in the user's hands.  (Source: Shinkai - Ask Me Anything.docx, Section: 2) 2024-05-05T00:33:00".to_string(), 97),
            SubPrompt::Content(SubPromptType::User, "tell me more about Shinkai. Answer the question using this markdown and the extra context provided: \n # Answer \n here goes the answer\n".to_string(), 100),
            SubPrompt::ToolAvailable(SubPromptType::AvailableTool, shinkai_tool.json_function_call_format().expect("mh"), 98),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let (messages, _token_length) =
            prompt.generate_chat_completion_messages(None, &ModelCapabilitiesManager::num_tokens_from_llama3);

        // Expected messages
        let expected_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Use the content to directly answer the user's question. If the user talks about `it` or `this`, they are referencing the previous message.\n Respond using the following markdown schema and nothing else:\n # Answer \nhere goes the answer\n".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("summarize this".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("assistant".to_string()),
                content: Some("## What are the benefits of using Vector Resources ...\n\n".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("tell me more about Shinkai. Answer the question using this markdown and the extra context provided: \n # Answer \n here goes the answer\n".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: None,
                content: None,
                name: None,
                function_call: None,
                functions: Some(vec![FunctionDetails {
                    name: "concat_strings".to_string(),
                    description: "Concatenates 2 to 4 strings.".to_string(),
                    tool_router_key: Some("local:::__official_shinkai:::concat_strings".to_string()),
                    parameters: FunctionParameters {
                        type_: "object".to_string(),
                        properties: serde_json::json!({
                            "first_string": {
                                "type": "string",
                                "description": "The first string to concatenate"
                            },
                            "second_string": {
                                "type": "string",
                                "description": "The second string to concatenate"
                            },
                            "third_string": {
                                "type": "string",
                                "description": "The third string to concatenate (optional)"
                            },
                            "fourth_string": {
                                "type": "string",
                                "description": "The fourth string to concatenate (optional)"
                            }
                        }),
                        required: vec![
                            "first_string".to_string(),
                            "second_string".to_string()
                        ],
                    },
                }]),
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("Here is a list of relevant new content provided for you to potentially use while answering:\n- FAQ Shinkai Overview What's Shinkai? (Summary)  (Source: Shinkai - Ask Me Anything.docx, Section: ) 2024-05-05T00:33:00\n- Shinkai is a comprehensive super app designed to enhance how users interact with AI. It allows users to run AI locally, facilitating direct conversations with documents and managing files converted into AI embeddings for advanced semantic searches across user data. This local execution ensures privacy and efficiency, putting control directly in the user's hands.  (Source: Shinkai - Ask Me Anything.docx, Section: 2) 2024-05-05T00:33:00".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
        ];

        // Check if the generated messages match the expected messages
        assert_eq!(messages, expected_messages);
    }

    #[test]
    fn test_generate_llm_messages_with_function_call() {
        let concat_strings_desc = "Concatenates 2 to 4 strings.".to_string();
        let tool = RustTool::new(
            "concat_strings".to_string(),
            concat_strings_desc.clone(),
            {
                let mut params = Parameters::new();
                params.add_property(
                    "first_string".to_string(),
                    "string".to_string(),
                    "The first string to concatenate".to_string(),
                    true,
                    None,
                );
                params.add_property(
                    "second_string".to_string(),
                    "string".to_string(),
                    "The second string to concatenate".to_string(),
                    true,
                    None,
                );
                params.add_property(
                    "third_string".to_string(),
                    "string".to_string(),
                    "The third string to concatenate (optional)".to_string(),
                    false,
                    None,
                );
                params.add_property(
                    "fourth_string".to_string(),
                    "string".to_string(),
                    "The fourth string to concatenate (optional)".to_string(),
                    false,
                    None,
                );
                params
            },
            ToolOutputArg::empty(),
            None,
            "local:::__official_shinkai:::concat_strings".to_string(),
        );
        let shinkai_tool = ShinkaiTool::Rust(tool, true);

        let sub_prompts = vec![
            SubPrompt::Content(
                SubPromptType::System,
                "You are a very helpful assistant.".to_string(),
                98,
            ),
            SubPrompt::ToolAvailable(
                SubPromptType::AvailableTool,
                shinkai_tool.json_function_call_format().expect("mh"),
                98,
            ),
            SubPrompt::Content(
                SubPromptType::User,
                "concatenate hola and chao\n Answer the question using the extra context provided.".to_string(),
                100,
            ),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({"name": "concat_strings", "arguments": {"first_string": "hola", "second_string": "chao"}}),
                100,
            ),
            SubPrompt::FunctionCallResponse(
                SubPromptType::Function,
                serde_json::json!({"function_call": {"name": "concat_strings", "arguments": {"first_string": "hola", "second_string": "chao"}}, "response": "holachao"}),
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let (messages, _token_length) =
            prompt.generate_chat_completion_messages(None, &ModelCapabilitiesManager::num_tokens_from_llama3);

        // Expected messages
        let expected_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are a very helpful assistant.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: None,
                content: None,
                name: None,
                function_call: None,
                functions: Some(vec![FunctionDetails {
                    name: "concat_strings".to_string(),
                    description: "Concatenates 2 to 4 strings.".to_string(),
                    tool_router_key: Some("local:::__official_shinkai:::concat_strings".to_string()),
                    parameters: FunctionParameters {
                        type_: "object".to_string(),
                        properties: serde_json::json!({
                            "first_string": {
                                "type": "string",
                                "description": "The first string to concatenate"
                            },
                            "second_string": {
                                "type": "string",
                                "description": "The second string to concatenate"
                            },
                            "third_string": {
                                "type": "string",
                                "description": "The third string to concatenate (optional)"
                            },
                            "fourth_string": {
                                "type": "string",
                                "description": "The fourth string to concatenate (optional)"
                            }
                        }),
                        required: vec!["first_string".to_string(), "second_string".to_string()],
                    },
                }]),
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some(
                    "concatenate hola and chao\n Answer the question using the extra context provided.".to_string(),
                ),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("assistant".to_string()),
                content: None,
                name: None,
                function_call: Some(DetailedFunctionCall {
                    name: "concat_strings".to_string(),
                    arguments: "{\"first_string\":\"hola\",\"second_string\":\"chao\"}".to_string(),
                    id: None,
                }),
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("function".to_string()),
                content: Some("holachao".to_string()),
                name: Some("concat_strings".to_string()),
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
        ];

        match serde_json::to_string_pretty(&messages) {
            Ok(pretty_json) => eprintln!("messages JSON: {}", pretty_json),
            Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
        };

        match serde_json::to_string_pretty(&expected_messages) {
            Ok(pretty_json) => eprintln!("expected messages JSON: {}", pretty_json),
            Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
        };

        // Check if the generated messages match the expected messages
        assert_eq!(messages, expected_messages);
    }
}
