#[macro_use]
use lazy_static::lazy_static;
use llm::load_progress_callback_stdout as load_callback;
use llm::Model;

lazy_static! {
    static ref DEFAULT_MODEL_PATH: &'static str = "pythia-160m-q4_0.bin";
}

/// Generates embeddings for an input string using a given language model.
///
/// This function will use a provided model for generating embeddings. If no
/// model is provided, it will load a default model specified by
/// `DEFAULT_MODEL_PATH`.
///
/// # Parameters
/// - `model`: An optional boxed language model to be used for generating
///   embeddings.
/// - `input_string`: The input string for which embeddings are generated.
///
/// # Returns
/// A vector of `f32` representing the embeddings for the input string.
///
/// # Panics
/// This function will panic if it fails to load the default model when no model
/// is provided.
pub fn generate_embeddings(model: Option<Box<dyn Model>>, input_string: &str) -> Vec<f32> {
    let model = match model {
        Some(model) => model,
        None => llm::load_dynamic(
            Some(llm::ModelArchitecture::GptNeoX),
            std::path::Path::new(&*DEFAULT_MODEL_PATH),
            llm::TokenizerSource::Embedded,
            Default::default(),
            load_callback,
        )
        .unwrap_or_else(|err| panic!("Failed to load model: {}", err)),
    };

    let mut session = model.start_session(Default::default());
    let mut output_request = llm::OutputRequest {
        all_logits: None,
        embeddings: Some(Vec::new()),
    };
    let vocab = model.tokenizer();
    let beginning_of_sentence = true;
    let query_token_ids = vocab
        .tokenize(input_string, beginning_of_sentence)
        .unwrap()
        .iter()
        .map(|(_, tok)| *tok)
        .collect::<Vec<_>>();
    model.evaluate(&mut session, &query_token_ids, &mut output_request);
    output_request.embeddings.unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embeddings_generation() {
        let dog_embeddings = generate_embeddings(None, "dog");
        let cat_embeddings = generate_embeddings(None, "cat");

        assert_eq!(dog_embeddings, dog_embeddings);
        assert_ne!(dog_embeddings, cat_embeddings);
    }
}
