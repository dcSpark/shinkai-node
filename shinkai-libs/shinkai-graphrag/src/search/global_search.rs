use futures::future::join_all;
use polars::frame::DataFrame;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tiktoken_rs::tokenizer::Tokenizer;

use crate::context_builder::community_context::GlobalCommunityContext;
use crate::context_builder::context_builder::{ContextBuilderParams, ConversationHistory};
use crate::llm::llm::{BaseLLM, BaseLLMCallback, LLMParams, MessageType};
use crate::llm::utils::num_tokens;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub response: ResponseType,
    pub context_data: ContextData,
    pub context_text: ContextText,
    pub completion_time: f64,
    pub llm_calls: usize,
    pub prompt_tokens: usize,
}

#[derive(Debug, Clone)]
pub enum ResponseType {
    String(String),
    Dictionary(HashMap<String, serde_json::Value>),
    Dictionaries(Vec<HashMap<String, serde_json::Value>>),
    KeyPoints(Vec<KeyPoint>),
}

#[derive(Debug, Clone)]
pub enum ContextData {
    String(String),
    DataFrames(Vec<DataFrame>),
    Dictionary(HashMap<String, DataFrame>),
}

#[derive(Debug, Clone)]
pub enum ContextText {
    String(String),
    Strings(Vec<String>),
    Dictionary(HashMap<String, String>),
}

#[derive(Debug, Clone)]
pub struct KeyPoint {
    pub answer: String,
    pub score: i32,
}

pub struct GlobalSearchResult {
    pub response: ResponseType,
    pub context_data: ContextData,
    pub context_text: ContextText,
    pub completion_time: f64,
    pub llm_calls: usize,
    pub prompt_tokens: usize,
    pub map_responses: Vec<SearchResult>,
    pub reduce_context_data: ContextData,
    pub reduce_context_text: ContextText,
}

#[derive(Debug, Clone)]
pub struct GlobalSearchLLMCallback {
    response: Vec<String>,
    map_response_contexts: Vec<String>,
    map_response_outputs: Vec<SearchResult>,
}

impl GlobalSearchLLMCallback {
    pub fn new() -> Self {
        GlobalSearchLLMCallback {
            response: Vec::new(),
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
    context_builder: GlobalCommunityContext,
    token_encoder: Option<Tokenizer>,
    context_builder_params: ContextBuilderParams,
    reduce_system_prompt: String,
    response_type: String,
    allow_general_knowledge: bool,
    general_knowledge_inclusion_prompt: String,
    callbacks: Option<Vec<GlobalSearchLLMCallback>>,
    max_data_tokens: usize,
    map_llm_params: LLMParams,
    reduce_llm_params: LLMParams,
}

impl GlobalSearch {
    pub fn new(
        llm: Box<dyn BaseLLM>,
        context_builder: GlobalCommunityContext,
        token_encoder: Option<Tokenizer>,
        reduce_system_prompt: String,
        response_type: String,
        allow_general_knowledge: bool,
        general_knowledge_inclusion_prompt: String,
        json_mode: bool,
        callbacks: Option<Vec<GlobalSearchLLMCallback>>,
        max_data_tokens: usize,
        map_llm_params: LLMParams,
        reduce_llm_params: LLMParams,
        context_builder_params: ContextBuilderParams,
    ) -> Self {
        let mut map_llm_params = map_llm_params;

        if json_mode {
            map_llm_params
                .response_format
                .insert("type".to_string(), "json_object".to_string());
        } else {
            map_llm_params.response_format.remove("response_format");
        }

        GlobalSearch {
            llm,
            context_builder,
            token_encoder,
            context_builder_params,
            reduce_system_prompt,
            response_type,
            allow_general_knowledge,
            general_knowledge_inclusion_prompt,
            callbacks,
            max_data_tokens,
            map_llm_params,
            reduce_llm_params,
        }
    }

    pub async fn asearch(
        &self,
        query: String,
        _conversation_history: Option<ConversationHistory>,
    ) -> anyhow::Result<GlobalSearchResult> {
        // Step 1: Generate answers for each batch of community short summaries
        let start_time = Instant::now();
        let (context_chunks, context_records) = self
            .context_builder
            .build_context(self.context_builder_params.clone())
            .await?;

        let mut callbacks = match &self.callbacks {
            Some(callbacks) => {
                let mut llm_callbacks = Vec::new();
                for callback in callbacks {
                    let mut callback = callback.clone();
                    callback.on_map_response_start(context_chunks.clone());
                    llm_callbacks.push(callback);
                }
                Some(llm_callbacks)
            }
            None => None,
        };

        let map_responses: Vec<_> = join_all(
            context_chunks
                .iter()
                .map(|data| self._map_response_single_batch(data, &query, self.map_llm_params.clone())),
        )
        .await;

        let map_responses: Result<Vec<_>, _> = map_responses.into_iter().collect();
        let map_responses = map_responses?;

        callbacks = match &callbacks {
            Some(callbacks) => {
                let mut llm_callbacks = Vec::new();
                for callback in callbacks {
                    let mut callback = callback.clone();
                    callback.on_map_response_end(map_responses.clone());
                    llm_callbacks.push(callback);
                }
                Some(llm_callbacks)
            }
            None => None,
        };

        let map_llm_calls: usize = map_responses.iter().map(|response| response.llm_calls).sum();
        let map_prompt_tokens: usize = map_responses.iter().map(|response| response.prompt_tokens).sum();

        // Step 2: Combine the intermediate answers from step 2 to generate the final answer
        let reduce_response = self
            ._reduce_response(map_responses.clone(), &query, callbacks, self.reduce_llm_params.clone())
            .await?;

        Ok(GlobalSearchResult {
            response: reduce_response.response,
            context_data: ContextData::Dictionary(context_records),
            context_text: ContextText::Strings(context_chunks),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: map_llm_calls + reduce_response.llm_calls,
            prompt_tokens: map_prompt_tokens + reduce_response.prompt_tokens,
            map_responses,
            reduce_context_data: reduce_response.context_data,
            reduce_context_text: reduce_response.context_text,
        })
    }

    async fn _map_response_single_batch(
        &self,
        context_data: &str,
        query: &str,
        llm_params: LLMParams,
    ) -> anyhow::Result<SearchResult> {
        let start_time = Instant::now();
        let search_prompt = String::new();
        let mut search_messages = Vec::new();
        search_messages.push(HashMap::from([
            ("role".to_string(), "system".to_string()),
            ("content".to_string(), search_prompt.clone()),
        ]));
        search_messages.push(HashMap::from([
            ("role".to_string(), "user".to_string()),
            ("content".to_string(), query.to_string()),
        ]));

        let search_response = self
            .llm
            .agenerate(MessageType::Dictionary(search_messages), false, None, llm_params)
            .await?;

        let processed_response = self.parse_search_response(&search_response);

        Ok(SearchResult {
            response: ResponseType::KeyPoints(processed_response),
            context_data: ContextData::String(context_data.to_string()),
            context_text: ContextText::String(context_data.to_string()),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: 1,
            prompt_tokens: num_tokens(&search_prompt, self.token_encoder),
        })
    }

    fn parse_search_response(&self, search_response: &str) -> Vec<KeyPoint> {
        let parsed_elements: Value = serde_json::from_str(search_response).unwrap_or_default();

        if let Some(points) = parsed_elements.get("points") {
            if let Some(points) = points.as_array() {
                return points
                    .iter()
                    .map(|element| KeyPoint {
                        answer: element
                            .get("description")
                            .unwrap_or(&Value::String("".to_string()))
                            .to_string(),
                        score: element
                            .get("score")
                            .unwrap_or(&Value::Number(serde_json::Number::from(0)))
                            .as_i64()
                            .unwrap_or(0) as i32,
                    })
                    .collect::<Vec<KeyPoint>>();
            }
        }

        Vec::new()
    }

    async fn _reduce_response(
        &self,
        map_responses: Vec<SearchResult>,
        query: &str,
        callbacks: Option<Vec<GlobalSearchLLMCallback>>,
        llm_params: LLMParams,
    ) -> anyhow::Result<SearchResult> {
        let start_time = Instant::now();
        let mut key_points: Vec<HashMap<String, String>> = Vec::new();

        for (index, response) in map_responses.iter().enumerate() {
            if let ResponseType::Dictionaries(response_list) = &response.response {
                for element in response_list {
                    if let (Some(answer), Some(score)) = (element.get("answer"), element.get("score")) {
                        let mut point = HashMap::new();
                        point.insert("analyst".to_string(), (index + 1).to_string());
                        point.insert("answer".to_string(), answer.to_string());
                        point.insert("score".to_string(), score.to_string());
                        key_points.push(point);
                    }
                }
            }
        }

        let filtered_key_points: Vec<HashMap<String, String>> = key_points
            .into_iter()
            .filter(|point| point.get("score").unwrap().parse::<i32>().unwrap() > 0)
            .collect();

        if filtered_key_points.is_empty() && !self.allow_general_knowledge {
            return Ok(SearchResult {
                response: ResponseType::String("NO_DATA_ANSWER".to_string()),
                context_data: ContextData::String("".to_string()),
                context_text: ContextText::String("".to_string()),
                completion_time: start_time.elapsed().as_secs_f64(),
                llm_calls: 0,
                prompt_tokens: 0,
            });
        }

        let mut sorted_key_points = filtered_key_points;
        sorted_key_points.sort_by(|a, b| {
            b.get("score")
                .unwrap()
                .parse::<i32>()
                .unwrap()
                .cmp(&a.get("score").unwrap().parse::<i32>().unwrap())
        });

        let mut data: Vec<String> = Vec::new();
        let mut total_tokens = 0;
        for point in sorted_key_points {
            let mut formatted_response_data: Vec<String> = Vec::new();
            formatted_response_data.push(format!("----Analyst {}----", point.get("analyst").unwrap()));
            formatted_response_data.push(format!("Importance Score: {}", point.get("score").unwrap()));
            formatted_response_data.push(point.get("answer").unwrap().to_string());
            let formatted_response_text = formatted_response_data.join("\n");
            if total_tokens + num_tokens(&formatted_response_text, self.token_encoder) > self.max_data_tokens {
                break;
            }
            data.push(formatted_response_text.clone());
            total_tokens += num_tokens(&formatted_response_text, self.token_encoder);
        }
        let text_data = data.join("\n\n");

        let search_prompt = format!(
            "{}\n{}",
            self.reduce_system_prompt
                .replace("{report_data}", &text_data)
                .replace("{response_type}", &self.response_type),
            if self.allow_general_knowledge {
                self.general_knowledge_inclusion_prompt.clone()
            } else {
                String::new()
            }
        );

        let search_messages = vec![
            HashMap::from([
                ("role".to_string(), "system".to_string()),
                ("content".to_string(), search_prompt.clone()),
            ]),
            HashMap::from([
                ("role".to_string(), "user".to_string()),
                ("content".to_string(), query.to_string()),
            ]),
        ];

        let llm_callbacks = match callbacks {
            Some(callbacks) => {
                let mut llm_callbacks = Vec::new();
                for callback in callbacks {
                    llm_callbacks.push(BaseLLMCallback {
                        response: callback.response.clone(),
                    });
                }
                Some(llm_callbacks)
            }
            None => None,
        };

        let search_response = self
            .llm
            .agenerate(
                MessageType::Dictionary(search_messages),
                true,
                llm_callbacks,
                llm_params,
            )
            .await?;

        Ok(SearchResult {
            response: ResponseType::String(search_response),
            context_data: ContextData::String(text_data.clone()),
            context_text: ContextText::String(text_data),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: 1,
            prompt_tokens: num_tokens(&search_prompt, self.token_encoder),
        })
    }
}
