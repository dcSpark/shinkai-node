use futures::Future;
use pddl_parser::domain::action::Action;
use pddl_parser::domain::domain::Domain;
use pddl_parser::domain::typed_parameter::TypedParameter;
use pddl_parser::domain::typing::Type;
use pddl_parser::error::ParserError;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use std::{pin::Pin, sync::Arc, io::Cursor};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::execution::job_prompts::Prompt;

pub type ExecuteActionFn = fn(&Action, &mut SharedState) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>;

fn default_execute_action_fn() -> ExecuteActionFn {
    fn action_fn(action: &Action, state: &mut SharedState) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>> {
        let action_name = action.name.clone();
        Box::pin(async move {
            println!("Default action execution for {}", action_name);
            Ok(())
        })
    }
    action_fn
}

#[derive(Default, Clone)]
pub struct SharedState {
    html_fetched: Option<String>,
    links_extracted: Option<Vec<String>>,
    content_fetched: Option<String>,
    summary_generated: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub domain: Domain,
    pub state: SharedState,
    // #[serde(skip_serializing, skip_deserializing, default = "default_execute_action_fn")]
    pub execute_action: ExecuteActionFn,
}

impl Plan {
    pub fn process_plan(plan: Arc<Mutex<Plan>>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut plan_guard = plan.lock().await;
            for action in plan_guard.clone().domain.actions.iter() {
                match (plan_guard.execute_action)(action, &mut plan_guard.state).await {
                    Ok(_) => {
                        println!("Action {} executed successfully", action.name);
                    }
                    Err(_) => {
                        println!("Action {} failed", action.name);
                    }
                }
            }
        })
    }

    pub async fn create_plan(agent: SerializedAgent, description: String, location: String) -> Result<Plan, String> {
        // Create a prompt for the LLM
        let prompt = Prompt::new(format!("Generate a PDDL for a plan with description '{}' at location '{}'", description, location));

        // Perform the LLM inference and await its response
        let llm_response = inference_agent(agent, prompt).await;

        // Assume the LLM response is a PDDL string
        let pddl = match llm_response {
            Ok(json_value) => json_value["pddl"].as_str().unwrap_or_default().to_string(),
            Err(e) => return Err(format!("LLM error: {:?}", e)),
        };

        // Parse the PDDL string into a Domain
        let domain = match Domain::parse(Cursor::new(pddl)) {
            Ok(domain) => domain,
            Err(e) => return Err(format!("PDDL parsing error: {:?}", e)),
        };

        // Create a new Plan with the parsed Domain and a default SharedState
        let plan = Plan {
            domain,
            state: SharedState::default(),
            execute_action: default_execute_action_fn(),
        };

        Ok(plan)
    }
}