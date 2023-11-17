use futures::Future;
use pddl_ish_parser::models::domain::Domain;
use pddl_ish_parser::parser::action::Action;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use std::{io::Cursor, pin::Pin, sync::Arc};
use tokio::sync::Mutex;
use pddl_ish_parser::parser::problem_parser::parse_problem;
use pddl_ish_parser::parser::domain_parser::parse_domain;
use crate::agent::execution::job_prompts::Prompt;

pub type ExecuteActionFn =
    fn(&Action, &mut SharedPlanState) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>;

fn default_execute_action_fn() -> ExecuteActionFn {
    fn action_fn(
        action: &Action,
        state: &mut SharedPlanState,
    ) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>> {
        let action_name = action.name.clone();
        Box::pin(async move {
            println!("Default action execution for {}", action_name);
            Ok(())
        })
    }
    action_fn
}

#[derive(Default, Clone, Debug)]
pub struct SharedPlanState {
    html_fetched: Option<String>,
    links_extracted: Option<Vec<String>>,
    content_fetched: Option<String>,
    summary_generated: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ShinkaiPlan {
    pub domain: Domain,
    pub state: SharedPlanState,
    // #[serde(skip_serializing, skip_deserializing, default = "default_execute_action_fn")]
    pub execute_action: ExecuteActionFn,
}

#[derive(Debug)]
pub enum ShinkaiPlanError {
    PddlParsingError(String),
}

/*
Two Parts

Part A:
    - when creating a cron job, create a plan
    - go over the plan until completed
    - also create a cron based on users preference
    - should the cron be an actual job to create a cron?
Part B:
    -
*/

impl ShinkaiPlan {
    pub fn process_plan(plan: Arc<Mutex<ShinkaiPlan>>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut plan_guard = plan.lock().await;
            // for action in plan_guard.clone().domain.actions.iter() {
            //     match (plan_guard.execute_action)(action, &mut plan_guard.state).await {
            //         Ok(_) => {
            //             println!("Action {} executed successfully", action.name);
            //         }
            //         Err(_) => {
            //             println!("Action {} failed", action.name);
            //         }
            //     }
            // }
        })
    }

    pub fn validate_pddl_domain(pddl: String) -> Result<(), String> {
        eprintln!("Validating PDDL domain");
        match parse_domain(&pddl) {
            Ok((_, _)) => {
                eprintln!("OK");
                Ok(())
            }
            Err(e) => {
                let error_message = format!("PDDL parsing error: {:?}", e);
                eprintln!("{}", error_message);
                Err(error_message)
            }
        }
    }
    
    pub fn validate_pddl_problem(pddl: String) -> Result<(), String> {
        eprintln!("Validating PDDL problem");
        match parse_problem(&pddl) {
            Ok((_, _)) => {
                eprintln!("OK");
                Ok(())
            }
            Err(e) => {
                let error_message = format!("PDDL parsing error: {:?}", e);
                eprintln!("{}", error_message);
                Err(error_message)
            }
        }
    }
}
