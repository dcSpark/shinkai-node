use crate::{llm_provider::execution::chains::inference_chain_trait::InferenceChainContextTrait, managers::model_capabilities_manager::ModelCapabilitiesManager};
use shinkai_dsl::sm_executor::WorkflowError;
use std::any::Any;

pub fn split_text_for_llm(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    let input1 = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for input1".to_string()))?
        .clone();
    let input2 = args[1]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for input2".to_string()))?
        .clone();

    let agent = context.agent();
    let max_tokens = ModelCapabilitiesManager::get_max_input_tokens(&agent.model);
    
    let mut result = Vec::new();
    let current_text = input1.clone();
    let mut remaining_text = input2.clone();

    while !remaining_text.is_empty() {
        let combined_text = format!("{}{}", current_text, remaining_text);
        let token_count = (ModelCapabilitiesManager::count_tokens_from_message_llama3(&combined_text) as f64 * 1.2).ceil() as usize; // we multiply it by 1.2 to be safe

        if token_count <= max_tokens {
            result.push(combined_text);
            break;
        } else {
            let (part, rest) = split_text_at_token_limit(&remaining_text, max_tokens, token_count);
            result.push(format!("{}{}", current_text, part));
            remaining_text = rest;
        }
    }

    Ok(Box::new(result.join(":::")))
}

pub fn split_text_at_token_limit(text: &str, token_limit: usize, current_token_count: usize) -> (String, String) {
    // Calculate the safe limit using the rule of three
    let safe_limit = ((token_limit as f64 * 0.8) / current_token_count as f64 * text.len() as f64).ceil() as usize;

    // Split the text at the safe limit
    let part = text.chars().take(safe_limit).collect::<String>();
    let rest = text.chars().skip(safe_limit).collect::<String>();

    // Backtrack to the nearest period
    if let Some(pos) = part.rfind('.') {
        let (new_part, new_rest) = part.split_at(pos + 1);
        let part = new_part.to_string();
        let rest = format!("{}{}", new_rest, rest);
        return (part, rest);
    }

    (part, rest)
}
