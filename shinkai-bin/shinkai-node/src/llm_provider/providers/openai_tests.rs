#[cfg(test)]
mod tests {
    use super::super::openai::parse_openai_stream_chunk;
    use super::super::openai::PartialFunctionCall;
    use serde_json;
    use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
    use std::sync::Arc;
    use tokio;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_youtube_search() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
            id: None,
            call_type: None,
        };
        let tools = Some(vec![serde_json::json!({
            "name": "youtube_search_api",
            "tool_router_key": "test_router"
        })]);
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        // Each string in this vector represents a complete chunk that may contain multiple data entries
        let chunks = vec![
            // First chunk - Initial message with role and function call
            r#"data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_TvKBsG2K5gy1PIv9YX7PHOlb","type":"function","function":{"name":"youtube_search_api","arguments":""}}],"refusal":null},"logprobs":null,"finish_reason":null}]}"#,
            // Second chunk - Contains two data entries
            r#"data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\""}}]},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"search"}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Third chunk - Contains two data entries
            r#"data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"Query"}}]},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\":\""}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Fourth chunk - Contains two data entries
            r#"data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"steam"}}]},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":" deck"}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Fifth chunk - Contains three data entries including the finish_reason and [DONE]
            r#"data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"}"}}]},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLG3cyCxffdqTiCAs8akKN6rha71Y","object":"chat.completion.chunk","created":1744406312,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"tool_calls"}]}
data: [DONE]"#,
        ];

        for chunk in chunks {
            buffer.push_str(chunk);
            buffer.push('\n');
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &ws_manager,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Verify the function call
        assert_eq!(function_calls.len(), 1);
        let fc = &function_calls[0];
        assert_eq!(fc.name, "youtube_search_api");
        assert_eq!(fc.tool_router_key, Some("test_router".to_string()));
        assert_eq!(fc.id, Some("call_TvKBsG2K5gy1PIv9YX7PHOlb".to_string()));
        assert_eq!(fc.call_type, Some("function".to_string()));

        // Verify the arguments
        let expected_args = serde_json::json!({
            "searchQuery": "steam deck"
        });
        assert_eq!(serde_json::to_value(&fc.arguments).unwrap(), expected_args);
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_content() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
            id: None,
            call_type: None,
        };
        let tools = None;
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        let chunks = vec![
            // Initial role setup
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"role":"assistant","content":"","refusal":null},"logprobs":null,"finish_reason":null}]}"#,
            // Content chunks
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"I"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" found"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" the"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" top"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" video"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" related"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" to"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" the"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" "},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"\""},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"Steam"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Deck"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":".\""},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Here"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" are"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" the"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" details"},"logprobs":null,"finish_reason":null}]}"#,
            r####"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":":\n\n"},"logprobs":null,"finish_reason":null}]}
        data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"###"},"logprobs":null,"finish_reason":null}]}"####,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Video"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Title"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":":"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" ["},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"The"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Good"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" News"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"!"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Steam"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Deck"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":" Update"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"!"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"]("},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"https"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"://"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"www"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":".youtube"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":".com"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"/watch"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"?v"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"="},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"V"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"27"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"DQ"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"DM"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"s"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"Bg"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":"I"},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"content":")"},"logprobs":null,"finish_reason":null}]}
data: {"id":"chatcmpl-BLGOkzXc864uH3CZxRW9stxqzlpOl","object":"chat.completion.chunk","created":1744407622,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"stop"}]}"#,
            r#"data: [DONE]"#,
        ];

        for chunk in chunks {
            buffer.push_str(chunk);
            buffer.push('\n');
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &ws_manager,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Verify the accumulated content
        assert_eq!(
            response_text,
            "I found the top video related to the \"Steam Deck.\" Here are the details:\n\n### Video Title: [The Good News! Steam Deck Update!](https://www.youtube.com/watch?v=V27DQDMsBgI)"
        );
        assert!(function_calls.is_empty());
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_multiple_tool_calls() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
            id: None,
            call_type: None,
        };
        let tools = Some(vec![
            serde_json::json!({
                "name": "getCurrentTemperature",
                "tool_router_key": "weather_router"
            }),
            serde_json::json!({
                "name": "getRainProbability",
                "tool_router_key": "weather_router"
            }),
        ]);
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        // Chunks representing streaming tool calls
        let chunks = vec![
            // Initial role setup
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"role":"assistant","content":null},"logprobs":null,"finish_reason":null}]}"#,
            // First function call initialization
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_pv0MOGlxgvBQlk3j6BFOg85Z","type":"function","function":{"name":"getCurrentTemperature","arguments":""}}]},"logprobs":null,"finish_reason":null}]}"#,
            // First function arguments streaming
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"lo"}}]},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"cation\": \"S"}}]},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"an Francisco, CA\", "}}]},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"unit\": \"Celsius\"}"}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Second function call initialization
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_nf6JwIChkn3z0iuCy7QWWo4M","type":"function","function":{"name":"getRainProbability","arguments":""}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Second function arguments streaming
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"lo"}}]},"logprobs":null,"finish_reason":null}]}"#,
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"cation\": \"San Francisco, CA\"}"}}]},"logprobs":null,"finish_reason":null}]}"#,
            // Completion
            r#"data: {"id":"chatcmpl-BLHRiX8Ho0oiVME8vV50mvcEXG7ue","object":"chat.completion.chunk","created":1744411650,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_44added55e","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"tool_calls"}]}"#,
            r#"data: [DONE]"#,
        ];

        for chunk in chunks {
            buffer.push_str(chunk);
            buffer.push('\n');
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &ws_manager,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Verify the function calls
        assert_eq!(function_calls.len(), 2);

        // Verify first function call
        let fc1 = &function_calls[0];
        assert_eq!(fc1.name, "getCurrentTemperature");
        assert_eq!(fc1.tool_router_key, Some("weather_router".to_string()));
        assert_eq!(fc1.id, Some("call_pv0MOGlxgvBQlk3j6BFOg85Z".to_string()));
        assert_eq!(fc1.call_type, Some("function".to_string()));

        let expected_args1 = serde_json::json!({});
        assert_eq!(serde_json::to_value(&fc1.arguments).unwrap(), expected_args1);

        // Verify second function call
        let fc2 = &function_calls[1];
        assert_eq!(fc2.name, "getRainProbability");
        assert_eq!(fc2.tool_router_key, Some("weather_router".to_string()));
        assert_eq!(fc2.id, Some("call_nf6JwIChkn3z0iuCy7QWWo4M".to_string()));
        assert_eq!(fc2.call_type, Some("function".to_string()));

        let expected_args2 = serde_json::json!({
            "location": "San Francisco, CA",
            "unit": "Celsius"
        });
        assert_eq!(serde_json::to_value(&fc2.arguments).unwrap(), expected_args2);

        // Verify no response text since content was null
        assert_eq!(response_text, "");
    }
}
