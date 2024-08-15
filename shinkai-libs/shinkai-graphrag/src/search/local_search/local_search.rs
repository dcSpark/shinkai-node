use std::{collections::HashMap, time::Instant};

use crate::{
    llm::llm::{BaseLLM, LLMParams, MessageType},
    search::base::{ContextData, ContextText, ResponseType},
};

use super::{
    mixed_context::{LocalSearchContextBuilderParams, LocalSearchMixedContext},
    prompts::LOCAL_SEARCH_SYSTEM_PROMPT,
};

pub struct LocalSearchResult {
    pub response: ResponseType,
    pub context_data: ContextData,
    pub context_text: ContextText,
    pub completion_time: f64,
    pub llm_calls: usize,
    pub prompt_tokens: usize,
}

pub struct LocalSearch {
    llm: Box<dyn BaseLLM>,
    context_builder: LocalSearchMixedContext,
    num_tokens_fn: fn(&str) -> usize,
    system_prompt: String,
    response_type: String,
    llm_params: LLMParams,
    context_builder_params: LocalSearchContextBuilderParams,
}

impl LocalSearch {
    pub fn new(
        llm: Box<dyn BaseLLM>,
        context_builder: LocalSearchMixedContext,
        num_tokens_fn: fn(&str) -> usize,
        llm_params: LLMParams,
        context_builder_params: LocalSearchContextBuilderParams,
        response_type: String,
        system_prompt: Option<String>,
    ) -> Self {
        let system_prompt = system_prompt.unwrap_or(LOCAL_SEARCH_SYSTEM_PROMPT.to_string());

        LocalSearch {
            llm,
            context_builder,
            num_tokens_fn,
            system_prompt,
            response_type,
            llm_params,
            context_builder_params,
        }
    }

    pub async fn asearch(&self, query: String) -> anyhow::Result<LocalSearchResult> {
        let start_time = Instant::now();
        let (context_text, context_records) = self
            .context_builder
            .build_context(self.context_builder_params.clone())
            .await?;

        let search_prompt = self
            .system_prompt
            .replace("{context_data}", &context_text)
            .replace("{response_type}", &self.response_type);

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
            .agenerate(
                MessageType::Dictionary(search_messages),
                false,
                None,
                self.llm_params.clone(),
            )
            .await?;

        Ok(LocalSearchResult {
            response: ResponseType::String(search_response),
            context_data: ContextData::Dictionary(context_records),
            context_text: ContextText::String(context_text),
            completion_time: start_time.elapsed().as_secs_f64(),
            llm_calls: 1,
            prompt_tokens: (self.num_tokens_fn)(&search_prompt),
        })
    }
}
