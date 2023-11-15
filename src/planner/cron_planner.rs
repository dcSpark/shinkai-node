use futures::Future;
use pddl::parsers::Span;
// use pddl_parser::domain::domain::Domain;
// use pddl_parser::domain::typed_parameter::TypedParameter;
// use pddl_parser::domain::typing::Type;
// use pddl_parser::error::ParserError;
use pddl_parser::{domain::action::Action, lexer::TokenStream};
use pddl::{Domain, Problem};
use pddl::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use std::{io::Cursor, pin::Pin, sync::Arc};
use tokio::sync::Mutex;

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
        let span = Span::new(&pddl);
        let parse_result = Domain::parse(span);
        match parse_result {
            Ok((remainder, _)) => {
                if remainder.fragment().is_empty() {
                    eprintln!("OK");
                    Ok(())
                } else {
                    let error_message = format!("PDDL parsing error: Unparsed remainder - {}", remainder.fragment());
                    eprintln!("{}", error_message);
                    Err(error_message)
                }
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
        let span = Span::new(&pddl);
        eprintln!("Parsing PDDL problem with span: {:?}", span);
        let parse_result = Problem::parse(span);
        match parse_result {
            Ok((remainder, _)) => {
                if remainder.fragment().is_empty() {
                    eprintln!("OK");
                    Ok(())
                } else {
                    let error_message = format!("PDDL parsing error: Unparsed remainder - {}", remainder.fragment());
                    eprintln!("{}", error_message);
                    Err(error_message)
                }
            }
            Err(e) => {
                let error_message = format!("PDDL parsing error: {:?}", e);
                eprintln!("{}", error_message);
                Err(error_message)
            }
        }
    }
}
