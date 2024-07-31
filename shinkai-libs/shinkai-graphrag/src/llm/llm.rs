use async_trait::async_trait;

pub struct BaseLLMCallback {
    response: Vec<String>,
}

impl BaseLLMCallback {
    pub fn new() -> Self {
        BaseLLMCallback { response: Vec::new() }
    }

    pub fn on_llm_new_token(&mut self, token: &str) {
        self.response.push(token.to_string());
    }
}

#[async_trait]
pub trait BaseLLM {
    async fn generate(&self, messages: Vec<String>, streaming: bool, callbacks: Option<Vec<BaseLLMCallback>>)
        -> String;

    async fn agenerate(
        &self,
        messages: Vec<String>,
        streaming: bool,
        callbacks: Option<Vec<BaseLLMCallback>>,
    ) -> String;
}

#[async_trait]
pub trait BaseTextEmbedding {
    async fn embed(&self, text: &str) -> Vec<f64>;

    async fn aembed(&self, text: &str) -> Vec<f64>;
}
