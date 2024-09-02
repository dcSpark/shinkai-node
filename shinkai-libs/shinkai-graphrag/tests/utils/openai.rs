use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, CreateChatCompletionRequestArgs,
        CreateEmbeddingRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use ndarray::{Array1, Array2, Axis};
use ndarray_stats::SummaryStatisticsExt;
use shinkai_graphrag::llm::base::{
    BaseLLM, BaseLLMCallback, BaseTextEmbedding, GlobalSearchPhase, LLMParams, MessageType,
};
use tiktoken_rs::{get_bpe_from_tokenizer, tokenizer::Tokenizer};

pub struct ChatOpenAI {
    pub api_key: Option<String>,
    pub model: String,
    pub max_retries: usize,
}

impl ChatOpenAI {
    pub fn new(api_key: Option<String>, model: &str, max_retries: usize) -> Self {
        ChatOpenAI {
            api_key,
            model: model.to_string(),
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

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(llm_params.max_tokens)
            .temperature(llm_params.temperature)
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

pub struct OpenAIEmbedding {
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: usize, // 8191
    pub max_retries: usize,
}

impl OpenAIEmbedding {
    pub fn new(api_key: Option<String>, model: &str, max_tokens: usize, max_retries: usize) -> Self {
        OpenAIEmbedding {
            api_key,
            model: model.to_string(),
            max_tokens,
            max_retries,
        }
    }

    async fn _aembed_with_retry(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut retry_count = 0;

        loop {
            match self._aembed(text).await {
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

    async fn _aembed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let client = match &self.api_key {
            Some(api_key) => Client::with_config(OpenAIConfig::new().with_api_key(api_key)),
            None => Client::new(),
        };

        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input([text.to_string()])
            .build()?;

        let response = client.embeddings().create(request).await?;
        let embedding = response
            .data
            .get(0)
            .map(|data| data.embedding.clone())
            .unwrap_or_default();

        Ok(embedding)
    }
}

#[async_trait]
impl BaseTextEmbedding for OpenAIEmbedding {
    async fn aembed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let token_chunks = chunk_text(text, self.max_tokens);
        let mut chunk_embeddings = Vec::new();
        let mut chunk_lens = Vec::new();

        for chunk in token_chunks {
            let embedding = self._aembed_with_retry(&chunk).await?;
            chunk_embeddings.push(embedding);
            chunk_lens.push(chunk.len());
        }

        if chunk_embeddings.len() == 1 {
            return Ok(chunk_embeddings.swap_remove(0));
        }

        let rows = chunk_embeddings.len();
        let cols = chunk_embeddings[0].len();
        let flat_embeddings: Vec<f32> = chunk_embeddings.into_iter().flatten().collect();
        let array_embeddings = Array2::from_shape_vec((rows, cols), flat_embeddings).unwrap();

        let array_lens = Array1::from_iter(chunk_lens.into_iter().map(|x| x as f32));

        // Calculate the weighted average
        let weighted_avg = array_embeddings.weighted_mean_axis(Axis(0), &array_lens).unwrap();

        // Normalize the embeddings
        let norm = weighted_avg.mapv(|x| x.powi(2)).sum().sqrt();
        let normalized_embeddings = weighted_avg / norm;

        Ok(normalized_embeddings.to_vec())
    }
}

pub fn num_tokens(text: &str) -> usize {
    let token_encoder = Tokenizer::Cl100kBase;
    let bpe = get_bpe_from_tokenizer(token_encoder).unwrap();
    bpe.encode_with_special_tokens(text).len()
}

fn batched<T>(iterable: impl Iterator<Item = T>, n: usize) -> impl Iterator<Item = Vec<T>> {
    if n < 1 {
        panic!("n must be at least one");
    }

    let mut it = iterable.peekable();
    std::iter::from_fn(move || {
        let mut batch = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(item) = it.next() {
                batch.push(item);
            } else {
                break;
            }
        }
        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    })
}

fn chunk_text<'a>(text: &'a str, max_tokens: usize) -> impl Iterator<Item = String> + 'a {
    let token_encoder = Tokenizer::Cl100kBase;
    let bpe = get_bpe_from_tokenizer(token_encoder).unwrap();
    let tokens = bpe.encode_with_special_tokens(text);

    let chunk_iterator = batched(tokens.into_iter(), max_tokens);
    chunk_iterator.map(move |chunk| bpe.decode(chunk).unwrap())
}
