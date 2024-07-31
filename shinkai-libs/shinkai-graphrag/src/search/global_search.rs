use futures::future::join_all;
//use polars::frame::DataFrame;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tiktoken::encoding::Encoding;
use tokio::sync::Semaphore;

use crate::context_builder::context_builder::{ConversationHistory, GlobalContextBuilder};
use crate::llm::llm::BaseLLM;

// TODO: Serialize and Deserialize polars::frame::DataFrame
type DataFrame = Vec<u8>;

#[derive(Debug, Serialize, Deserialize)]
struct SearchResult {
    response: ResponseType,
    context_data: ContextData,
    context_text: ContextText,
    completion_time: f64,
    llm_calls: u32,
    prompt_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseType {
    String(String),
    Dictionary(HashMap<String, serde_json::Value>),
    Dictionaries(Vec<HashMap<String, serde_json::Value>>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ContextData {
    String(String),
    DataFrames(Vec<DataFrame>),
    Dictionary(HashMap<String, DataFrame>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ContextText {
    String(String),
    Strings(Vec<String>),
    Dictionary(HashMap<String, String>),
}

#[derive(Serialize, Deserialize)]
pub struct GlobalSearchResult {
    response: ResponseType,
    context_data: ContextData,
    context_text: ContextText,
    completion_time: f64,
    llm_calls: i32,
    prompt_tokens: i32,
    map_responses: Vec<SearchResult>,
    reduce_context_data: ContextData,
    reduce_context_text: ContextText,
}

struct GlobalSearchLLMCallback {
    map_response_contexts: Vec<String>,
    map_response_outputs: Vec<SearchResult>,
}

impl GlobalSearchLLMCallback {
    pub fn new() -> Self {
        GlobalSearchLLMCallback {
            map_response_contexts: Vec::new(),
            map_response_outputs: Vec::new(),
        }
    }

    pub fn on_map_response_start(&mut self, map_response_contexts: Vec<String>) {
        self.map_response_contexts = map_response_contexts;
    }

    pub fn on_map_response_end(&mut self, map_response_outputs: Vec<SearchResult>) {
        self.map_response_outputs = map_response_outputs;
    }
}

pub struct GlobalSearch {
    llm: Box<dyn BaseLLM>,
    context_builder: Box<dyn GlobalContextBuilder>,
    token_encoder: Option<Encoding>,
    llm_params: Option<HashMap<String, serde_json::Value>>,
    context_builder_params: Option<HashMap<String, serde_json::Value>>,
    map_system_prompt: String,
    reduce_system_prompt: String,
    response_type: String,
    allow_general_knowledge: bool,
    general_knowledge_inclusion_prompt: String,
    callbacks: Option<Vec<GlobalSearchLLMCallback>>,
    max_data_tokens: usize,
    map_llm_params: HashMap<String, serde_json::Value>,
    reduce_llm_params: HashMap<String, serde_json::Value>,
    semaphore: Semaphore,
}

impl GlobalSearch {
    pub fn new(
        llm: Box<dyn BaseLLM>,
        context_builder: Box<dyn GlobalContextBuilder>,
        token_encoder: Option<Encoding>,
        map_system_prompt: String,
        reduce_system_prompt: String,
        response_type: String,
        allow_general_knowledge: bool,
        general_knowledge_inclusion_prompt: String,
        json_mode: bool,
        callbacks: Option<Vec<GlobalSearchLLMCallback>>,
        max_data_tokens: usize,
        map_llm_params: HashMap<String, serde_json::Value>,
        reduce_llm_params: HashMap<String, serde_json::Value>,
        context_builder_params: Option<HashMap<String, serde_json::Value>>,
        concurrent_coroutines: usize,
    ) -> Self {
        let mut map_llm_params = map_llm_params;

        if json_mode {
            map_llm_params.insert(
                "response_format".to_string(),
                serde_json::json!({"type": "json_object"}),
            );
        } else {
            map_llm_params.remove("response_format");
        }

        let semaphore = Semaphore::new(concurrent_coroutines);

        GlobalSearch {
            llm,
            context_builder,
            token_encoder,
            llm_params: None,
            context_builder_params,
            map_system_prompt,
            reduce_system_prompt,
            response_type,
            allow_general_knowledge,
            general_knowledge_inclusion_prompt,
            callbacks,
            max_data_tokens,
            map_llm_params,
            reduce_llm_params,
            semaphore,
        }
    }

    pub async fn asearch(
        &self,
        query: String,
        conversation_history: Option<ConversationHistory>,
    ) -> GlobalSearchResult {
        // Step 1: Generate answers for each batch of community short summaries
        let start_time = Instant::now();
        let (context_chunks, context_records) = self
            .context_builder
            .build_context(conversation_history, self.context_builder_params)
            .await;

        if let Some(callbacks) = &self.callbacks {
            for callback in callbacks {
                callback.on_map_response_start(context_chunks);
            }
        }

        let map_responses: Vec<_> = join_all(
            context_chunks
                .iter()
                .map(|data| self._map_response_single_batch(data, &query, &self.map_llm_params)),
        )
        .await;

        if let Some(callbacks) = &self.callbacks {
            for callback in callbacks {
                callback.on_map_response_end(&map_responses);
            }
        }

        let map_llm_calls: usize = map_responses.iter().map(|response| response.llm_calls).sum();
        let map_prompt_tokens: usize = map_responses.iter().map(|response| response.prompt_tokens).sum();

        // Step 2: Combine the intermediate answers from step 2 to generate the final answer
        let reduce_response = self
            ._reduce_response(&map_responses, &query, self.reduce_llm_params)
            .await;

        GlobalSearchResult {
            response: reduce_response.response,
            context_data: ContextData::Dictionary(context_records),
            context_text: ContextText::Strings(context_chunks),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: map_llm_calls + reduce_response.llm_calls,
            prompt_tokens: map_prompt_tokens + reduce_response.prompt_tokens,
            map_responses,
            reduce_context_data: reduce_response.context_data,
            reduce_context_text: reduce_response.context_text,
        }
    }

    async fn _reduce_response(
        &self,
        map_responses: Vec<SearchResult>,
        query: &str,
        reduce_llm_params: HashMap<String, serde_json::Value>,
    ) -> SearchResult {
        let start_time = Instant::now();
        let mut key_points = Vec::new();

        for (index, response) in map_responses.iter().enumerate() {
            if let ResponseType::Dictionaries(response_list) = response.response {
                for element in response_list {
                    if let (Some(answer), Some(score)) = (element.get("answer"), element.get("score")) {
                        key_points.push((index, answer.clone(), score.clone()));
                    }
                }
            }
        }

        let filtered_key_points: Vec<_> = key_points
            .into_iter()
            .filter(|(_, _, score)| score.as_f64().unwrap_or(0.0) > 0.0)
            .collect();

        if filtered_key_points.is_empty() && !self.allow_general_knowledge {
            return SearchResult {
                response: ResponseType::String("NO_DATA_ANSWER".to_string()),
                context_data: ContextData::String("".to_string()),
                context_text: ContextText::String("".to_string()),
                completion_time: start_time.elapsed().as_secs_f64(),
                llm_calls: 0,
                prompt_tokens: 0,
            };
        }

        let mut sorted_key_points = filtered_key_points;
        sorted_key_points.sort_by(|a, b| {
            b.2.as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a.2.as_f64().unwrap_or(0.0))
                .unwrap()
        });

        // TODO: Implement rest of the function

        SearchResult {
            response: ResponseType::String("Combined response".to_string()),
            context_data: ContextData::String("".to_string()),
            context_text: ContextText::String("".to_string()),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: 0,
            prompt_tokens: 0,
        }
    }
}
