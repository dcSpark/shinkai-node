use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionResponseFormat,
        ChatCompletionResponseFormatType, CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use shinkai_graphrag::llm::llm::{BaseLLM, BaseLLMCallback, GlobalSearchPhase, LLMParams, MessageType};
use tiktoken_rs::{get_bpe_from_tokenizer, tokenizer::Tokenizer};

pub struct ChatOpenAI {
    pub api_key: Option<String>,
    pub model: String,
    pub max_retries: usize,
}

impl ChatOpenAI {
    pub fn new(api_key: Option<String>, model: String, max_retries: usize) -> Self {
        ChatOpenAI {
            api_key,
            model,
            max_retries,
        }
    }

    pub async fn agenerate(
        &self,
        messages: MessageType,
        streaming: bool,
        callbacks: Option<Vec<BaseLLMCallback>>,
        llm_params: LLMParams,
    ) -> anyhow::Result<String> {
        let mut retry_count = 0;

        loop {
            match self
                ._agenerate(messages.clone(), streaming, callbacks.clone(), llm_params.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if retry_count < self.max_retries {
                        retry_count += 1;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    async fn _agenerate(
        &self,
        messages: MessageType,
        _streaming: bool,
        _callbacks: Option<Vec<BaseLLMCallback>>,
        llm_params: LLMParams,
    ) -> anyhow::Result<String> {
        let client = match &self.api_key {
            Some(api_key) => Client::with_config(OpenAIConfig::new().with_api_key(api_key)),
            None => Client::new(),
        };

        let messages = match messages {
            MessageType::String(message) => vec![message],
            MessageType::Strings(messages) => messages,
            MessageType::Dictionary(messages) => {
                let messages = messages
                    .iter()
                    .map(|message_map| {
                        message_map
                            .iter()
                            .map(|(key, value)| format!("{}: {}", key, value))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .collect();
                messages
            }
        };

        let request_messages = messages
            .into_iter()
            .map(|m| ChatCompletionRequestSystemMessageArgs::default().content(m).build())
            .collect::<Vec<_>>();

        let request_messages: Result<Vec<_>, _> = request_messages.into_iter().collect();
        let request_messages = request_messages?;
        let request_messages = request_messages
            .into_iter()
            .map(|m| Into::<ChatCompletionRequestMessage>::into(m.clone()))
            .collect::<Vec<ChatCompletionRequestMessage>>();

        let _response_format = if llm_params
            .response_format
            .get_key_value("type")
            .is_some_and(|(_k, v)| v == "json_object")
        {
            ChatCompletionResponseFormat {
                r#type: ChatCompletionResponseFormatType::JsonObject,
            }
        } else {
            ChatCompletionResponseFormat {
                r#type: ChatCompletionResponseFormatType::Text,
            }
        };

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(llm_params.max_tokens)
            .temperature(llm_params.temperature)
            //.response_format(response_format)
            .model(self.model.clone())
            .messages(request_messages)
            .build()?;

        let response = client.chat().create(request).await?;

        if let Some(choice) = response.choices.get(0) {
            return Ok(choice.message.content.clone().unwrap_or_default());
        }

        return Ok(String::new());
    }
}

#[async_trait]
impl BaseLLM for ChatOpenAI {
    async fn agenerate(
        &self,
        messages: MessageType,
        streaming: bool,
        callbacks: Option<Vec<BaseLLMCallback>>,
        llm_params: LLMParams,
        _search_phase: Option<GlobalSearchPhase>,
    ) -> anyhow::Result<String> {
        self.agenerate(messages, streaming, callbacks, llm_params).await
    }
}

pub fn num_tokens(text: &str) -> usize {
    let token_encoder = Tokenizer::Cl100kBase;
    let bpe = get_bpe_from_tokenizer(token_encoder).unwrap();
    bpe.encode_with_special_tokens(text).len()
}
