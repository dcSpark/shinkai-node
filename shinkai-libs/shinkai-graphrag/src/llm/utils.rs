use tiktoken_rs::{get_bpe_from_tokenizer, tokenizer::Tokenizer};

pub fn num_tokens(text: &str, token_encoder: Option<Tokenizer>) -> usize {
    let token_encoder = token_encoder.unwrap_or_else(|| Tokenizer::Cl100kBase);
    let bpe = get_bpe_from_tokenizer(token_encoder).unwrap();
    bpe.encode_with_special_tokens(text).len()
}
