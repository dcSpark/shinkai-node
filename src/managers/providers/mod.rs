pub mod openai;

pub trait Provider {
    type Response;
    fn parse_response(response_body: &str) -> Result<Self::Response, Box<dyn std::error::Error>>;
    fn extract_content(response: &Self::Response) -> String;
}
