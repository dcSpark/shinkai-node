#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use reqwest::Client;
    use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, GenericAPI, OpenAI};
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
    use shinkai_node::agent::execution::prompts::prompts::Prompt;
    use shinkai_node::agent::providers::LLMService;
    use shinkai_node::agent::{
        execution::prompts::prompts::{JobPromptGenerator, SubPrompt, SubPromptType},
        job_manager::JobManager,
        parsing_helper::ParsingHelper,
    };
    use tokio;

    fn setup_vars() -> Result<(AgentLLMInterface, Client, Option<String>, Option<String>), &'static str> {
        // Extract from ENV with fallback to default values
        let model_type =
            env::var("INITIAL_AGENT_MODEL").unwrap_or_else(|_| "togethercomputer/llama-2-70b-chat".to_string());
        let client = Client::new();
        let api_key = env::var("INITIAL_AGENT_API_KEY").ok().map(|key| key.to_string());

        if api_key.is_none() {
            return Err("No API key provided");
        }

        // Determine the provider type from ENV or default to GenericAPI
        let provider_type = env::var("INITIAL_TEST_LLM_PROVIDER").unwrap_or_else(|_| "genericapi".to_string());

        // Create an instance of AgentLLMInterface based on the provider type
        let provider = match provider_type.as_str() {
            "openai" => AgentLLMInterface::OpenAI(OpenAI { model_type }),
            _ => AgentLLMInterface::GenericAPI(GenericAPI { model_type }),
        };

        let url = env::var("INITIAL_AGENT_URL")
            .ok()
            .map(|url| url.to_string())
            .or_else(|| Some("https://api.together.xyz".to_string()));

        Ok((provider, client, url, api_key))
    }

    #[tokio::test]
    async fn test_call_llm_with_prompts_case_a() {
        init_default_tracing();
        match setup_vars() {
            Ok((provider, client, url, api_key)) => {
                let elements_list: Vec<Vec<String>> =
                    vec![get_elements_whats_zeko_with_6_resp() /* add more elements here */];

                for elements in elements_list {
                    let prompt = JobPromptGenerator::simple_doc_description(elements);
                    test_call_api(provider.clone(), &client, url.as_ref(), api_key.as_ref(), prompt).await;
                }
            }
            Err(e) => {
                eprintln!("Skipping test due to setup error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_call_llm_with_prompts_case_b() {
        init_default_tracing();
        match setup_vars() {
            Ok((provider, client, url, api_key)) => {
                let elements_list: Vec<String> = vec![get_zeko_description() /* add more elements here */];

                for elements in elements_list {
                    let prompt = JobPromptGenerator::qa_response_prompt_with_vector_search_final(
                        "What's Zeko?".to_string(),
                        vec![],
                        Some(elements),
                        None,
                        1,
                        32000,
                    );

                    test_call_api(provider.clone(), &client, url.as_ref(), api_key.as_ref(), prompt).await;
                }
            }
            Err(e) => {
                eprintln!("Skipping test due to setup error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_call_llm_with_simple_prompt() {
        init_default_tracing();
        match setup_vars() {
            Ok((provider, client, url, api_key)) => {
                let elements_list: Vec<String> = vec![
                    "Hello!".to_string(),
                    "what does it mean to have 70B parameters?".to_string(),
                ];

                for element in elements_list {
                    let prompt = JobPromptGenerator::qa_response_prompt_with_vector_search_final(
                        element.to_string(),
                        vec![],
                        Some("".to_string()),
                        None,
                        1,
                        32000,
                    );

                    test_call_api(provider.clone(), &client, url.as_ref(), api_key.as_ref(), prompt).await;
                }
            }
            Err(e) => {
                eprintln!("Skipping test due to setup error: {}", e);
            }
        }
    }

    async fn test_call_api(
        provider: AgentLLMInterface,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) {
        for _ in 0..3 {
            let result = match &provider {
                AgentLLMInterface::OpenAI(openai) => {
                    openai
                        .call_api(client, url, api_key, prompt.clone(), provider.clone())
                        .await
                }
                AgentLLMInterface::GenericAPI(genericapi) => {
                    genericapi
                        .call_api(client, url, api_key, prompt.clone(), provider.clone())
                        .await
                }
                _ => unimplemented!(), // or handle other cases as needed
            };

            match result {
                Ok(value) => {
                    eprintln!("Partial result: {:?}", value);
                    let extraction_key = "answer";
                    let json_result = JobManager::direct_extract_key_inference_response(value, extraction_key);
                    assert!(json_result.is_ok());
                }
                Err(e) => panic!("API call failed: {:?}", e),
            }
        }
    }

    #[tokio::test]
    async fn test_removing_low_priority_prompts() {
        init_default_tracing();

        let elements_list: Vec<Vec<String>> =
            vec![get_elements_whats_zeko_with_6_resp() /* add more elements here */];

        for elements in elements_list {
            let mut prompt = JobPromptGenerator::simple_doc_description(elements);

            for sp in &prompt.sub_prompts {
                eprintln!("Subprompt: {:?}", sp);
            }

            eprintln!(
                "Before removing len: {} - lowest priority: {:?}---------------------------\n",
                prompt.sub_prompts.len(),
                prompt.lowest_priority
            );
            for i in 0..prompt.sub_prompts.len() {
                eprintln!("Iteration count: {}", i);
                prompt.remove_lowest_priority_sub_prompt();
                eprintln!(
                    "After removing len: {} - lowest priority: {:?}\n",
                    &prompt.sub_prompts.len(),
                    prompt.lowest_priority
                );
                for sp in &prompt.sub_prompts {
                    eprintln!("Subprompt: {:?}", sp);
                }
            }

            eprintln!("\n--------------------------------------------------------");
            for sp in prompt.sub_prompts {
                eprintln!("Subprompt: {:?}", sp);
            }
        }
    }

    pub fn get_elements_whats_zeko_with_6_resp() -> Vec<String> {
        vec![
            "Zeko: Fractal scaling of ZK applications using a Shared Sequencer L2 Stack Robert Kornacki rob@milkomeda.com Nicolas Arqueros nico@milkomeda.com Sebastien Guillemot seba@milkomeda.com Brandon Kase bkase@o1labs.org Florian Kluge florian.kluge@o1labs.org October 5, 2023 1 Introduction ".to_string(), 
            "With the latest innovations in the blockchain space pushing towards a rollup-dominant future, Mina has found itself to be positioned in an ideal place to capitalize on all of its upfront ZK work, but now in a new direction. At this point in time ZK Rollups have reached significant mind-share in the blockchain industry as the ideal scaling solution, yet due to the inherent complexity at play, are up till now a nascent and burgeoning field with no clear winner. ".to_string(),
            "A ZK Rollup on top of Mina unlocks the best of what Mina’s model has to offer while maintaining a competitive edge with the likes of Ethereum and other leading chains with growing L2 ecosystems. Furthermore, this project seeks to strengthen Mina’s strong points by fulfilling the following goals: ".to_string(),
            "Increasing the throughput of Mina L1 Unlocking new possibilities for dApps by offering a DA Layer as a part of the L2 Implementing reusable rollup architecture that can be expanded upon by future innovative Rollups (both onto Mina and in the future to Ethereum as well) ".to_string(),
            "Improve UX by supporting faster block times at the L2 level 2 Motivation ".to_string(),
            "Mina Protocol is a layer one blockchain that aims to be the privacy and security layer for web3 through utilization of zero knowledge proofs. Mina itself is powered by a multi-tiered recursive zkSNARK proof that in a small, constant, size (succinctness) stands in for the full blockchain. Developers write smart contracts on top of Mina by tapping into this proof layer: The zkApps protocol extends the potential of zero-knowledge cryptography by enabling the following characteristics while preserving the succinctness of Mina: ".to_string(),
            ]
    }

    pub fn get_zeko_description() -> String {
        "Here is a list of relevant new content the user provided for you to use while answering: ``` - Fractal scaling, ZK applications, Shared Sequencer L2 Stack, Mina, rollup-dominant future, ZK Rollups, blockchain industry, nascent field, competitive edge, Ethereum, L2 ecosystems, increasing throughput, unlocking possibilities, DA Layer, reusable rollup architecture, faster block times, privacy, security, web3, zero-knowledge proofs, smart contracts, zkApps protocol, succinctness, recursive zkSNARK proof, layer one blockchain (Source: files/Zeko_Mina_Rollup.pdf)\n\n - Zeko: Fractal scaling of ZK applications using a Shared Sequencer L2 Stack Robert Kornacki rob@milkomeda.com Nicolas Arqueros nico@milkomeda.com Sebastien Guillemot seba@milkomeda.com Brandon Kase bkase@o1labs.org Florian Kluge florian.kluge@o1labs.org October 5, 2023 Introduction (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [1])\n\n - As more zkApps are expected to live at the L2 level rather than the L1, this means the majority of useful state that users will want to bridge over to Ethereum will be on the L2. As such it will be important to ensure that the bridges built for the L1 can also be used directly with our L2 to make the developer experience one-to-one, no matter on what layer the zkApp is deployed on. (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [16])\n\n - 12.7 Connect ZK Rollup To Mina Bridges (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [])\n\n - With the latest innovations in the blockchain space pushing towards a rollup-dominant future, Mina has found itself to be positioned in an ideal place to capitalize on all of its upfront ZK work, but now in a new direction. At this point in time ZK Rollups have reached significant mind-share in the blockchain industry as the ideal scaling solution, yet due to the inherent complexity at play, are (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [1])\n\n - The ZK Rollup L2 node will function similarly to an L1 Mina node, however with a few key differences. (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [11])\n\n - 8 L2 (ZK Rollup) Node (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [])\n\n - A ZK Rollup on top of Mina unlocks the best of what Mina’s model has to offer while maintaining a competitive edge with the likes of Ethereum and other leading chains with growing L2 ecosystems. Furthermore, this project seeks to strengthen Mina’s strong points by fulfilling the following goals: Increasing the throughput of Mina L1 (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [1])\n\n - 8. Native zkApp execution environment: Developers should be able to reuse zkApp native functional- ities and tooling, as both Mina and the rollup should have equivalent execution environments and not break native smart contracts. 4 Protocol Summary (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [2, 3])\n\n - In order for the ZK Rollup to be sustainable, it must have its own token as an integral part of the fee structure, both for the L2 itself and for the DA layer. These fees map onto work (computation), DA (networking + storage), and other real costs which underlie the functioning of the L2 itself. At a high level, the fees in the system are based off of: (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [12])\n\n - In order to fulfill the above stated goals in the Introduction, this ZK Rollup project intends to adhere to the following requirements: 1. Minimize missing rollup blocks: Mina has a very long block time, and so missing a rollup block (ex: no sequencer elected for that slot) has a large impact on the L2 throughput (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [2])\n\n - Altho (Source: files/Zeko_Mina_Rollup.pdf, Pgs: [18])\n\n ```.\n".to_string()
    }
}
