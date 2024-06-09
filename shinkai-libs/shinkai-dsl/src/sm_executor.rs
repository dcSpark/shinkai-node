use std::collections::HashMap;
use std::pin::Pin;
use std::{any::Any, fmt};

use async_trait::async_trait;
use futures::Future;

use crate::dsl_schemas::{
    Action, ComparisonOperator, Expression, FunctionCall, Param, StepBody, Workflow, WorkflowValue,
};

/*
TODOs:
- we want to return all the steps that were executed, not just the final registers (this is for step_history)
- we want to return specific errors
- logging + feedback for the user + feedback for workflow devs
- let's start with basic fn like inference
- we can have another fn that's a more custom inference
 */

#[derive(Debug)]
pub enum WorkflowError {
    FunctionError(String),
    EvaluationError(String),
    ExecutionError(String),
    InvalidArgument(String),
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkflowError::FunctionError(msg) => write!(f, "Function error: {}", msg),
            WorkflowError::EvaluationError(msg) => write!(f, "Evaluation error: {}", msg),
            WorkflowError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            WorkflowError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
        }
    }
}

impl std::error::Error for WorkflowError {}

#[async_trait]
pub trait AsyncFunction: Send + Sync {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError>;
}

pub type FunctionMap<'a> = HashMap<String, Box<dyn AsyncFunction + 'a>>;

pub struct WorkflowEngine<'a> {
    functions: &'a FunctionMap<'a>,
}

pub struct StepExecutor<'a> {
    engine: &'a WorkflowEngine<'a>,
    workflow: &'a Workflow,
    pub current_step: usize,
    pub registers: HashMap<String, String>,
}

impl<'a> WorkflowEngine<'a> {
    pub fn new(functions: &'a FunctionMap<'a>) -> Self {
        WorkflowEngine { functions }
    }

    pub async fn execute_workflow(&self, workflow: &Workflow) -> Result<HashMap<String, String>, WorkflowError> {
        let mut registers = HashMap::new();
        for step in &workflow.steps {
            for body in &step.body {
                self.execute_step_body(body, &mut registers).await?;
            }
        }
        Ok(registers)
    }

    pub fn execute_step_body<'b>(
        &'b self,
        step_body: &'b StepBody,
        registers: &'b mut HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkflowError>> + Send + 'b>> {
        Box::pin(async move {
            match step_body {
                StepBody::Action(action) => self.execute_action(action, registers).await,
                StepBody::Condition { condition, body } => {
                    if self.evaluate_condition(condition, registers)? {
                        self.execute_step_body(body, registers).await?;
                    }
                    Ok(())
                }
                StepBody::ForLoop { var, in_expr, action } => {
                    if let Expression::Range { start, end } = in_expr {
                        let start = self.evaluate_param(start.as_ref(), registers).parse::<i32>().unwrap_or(0);
                        let end = self.evaluate_param(end.as_ref(), registers).parse::<i32>().unwrap_or(0);
                        for i in start..=end {
                            registers.insert(var.clone(), i.to_string());
                            self.execute_step_body(action, registers).await?;
                        }
                    }
                    Ok(())
                }
                StepBody::RegisterOperation { register, value } => {
                    println!("Setting register {} to {:?}", register, value);
                    let value = self.evaluate_workflow_value(value, registers).await;
                    println!("Value: {}", value);
                    registers.insert(register.clone(), value);
                    Ok(())
                }
                StepBody::Composite(bodies) => {
                    for body in bodies {
                        self.execute_step_body(body, registers).await?;
                    }
                    Ok(())
                }
            }
        })
    }

    pub async fn execute_action(
        &self,
        action: &Action,
        registers: &mut HashMap<String, String>,
    ) -> Result<(), WorkflowError> {
        println!("Executing action: {:?}", action);
        match action {
            Action::ExternalFnCall(FunctionCall { name, args }) => {
                if let Some(func) = self.functions.get(name) {
                    let arg_values = args
                        .iter()
                        .map(|arg| Box::new(self.evaluate_param(arg, registers)) as Box<dyn Any + Send>)
                        .collect();
                    let result = func.call(arg_values).await?;
                    if let Ok(result) = result.downcast::<String>() {
                        if let Some(Param::Identifier(register_name)) = args.first() {
                            registers.insert(register_name.clone(), (*result).clone());
                        }
                    }
                }
                Ok(())
            }
            _ => Err(WorkflowError::FunctionError(format!("Unhandled action: {:?}", action))),
        }
    }

    pub fn evaluate_condition(
        &self,
        expression: &Expression,
        registers: &HashMap<String, String>,
    ) -> Result<bool, WorkflowError> {
        match expression {
            Expression::Binary { left, operator, right } => {
                let left_val = self
                    .evaluate_param(left, registers)
                    .parse::<i32>()
                    .map_err(|_| WorkflowError::EvaluationError(format!("Failed to parse left operand: {:?}", left)))?;
                let right_val = self.evaluate_param(right, registers).parse::<i32>().map_err(|_| {
                    WorkflowError::EvaluationError(format!("Failed to parse right operand: {:?}", right))
                })?;
                let result = match operator {
                    ComparisonOperator::Less => left_val < right_val,
                    ComparisonOperator::Greater => left_val > right_val,
                    ComparisonOperator::Equal => left_val == right_val,
                    ComparisonOperator::NotEqual => left_val != right_val,
                    ComparisonOperator::LessEqual => left_val <= right_val,
                    ComparisonOperator::GreaterEqual => left_val >= right_val,
                };
                Ok(result)
            }
            _ => Err(WorkflowError::EvaluationError(
                "Unsupported expression type".to_string(),
            )),
        }
    }

    pub fn evaluate_param(&self, param: &Param, registers: &HashMap<String, String>) -> String {
        eprintln!("Evaluating param: {:?}", param);
        eprintln!("Registers: {:?}", registers);
        match param {
            Param::Number(n) => n.to_string(),
            Param::Identifier(id) | Param::Register(id) => registers.get(id).cloned().unwrap_or_else(|| {
                eprintln!(
                    "Warning: Identifier/Register '{}' not found in registers, defaulting to 0",
                    id
                );
                "0".to_string()
            }),
            _ => {
                eprintln!("Warning: Unsupported parameter type, defaulting to 0");
                "0".to_string()
            }
        }
    }

    pub async fn evaluate_workflow_value(&self, value: &WorkflowValue, registers: &HashMap<String, String>) -> String {
        match value {
            WorkflowValue::Number(n) => n.to_string(),
            WorkflowValue::Identifier(id) => registers.get(id).cloned().unwrap_or_else(|| "0".to_string()),
            WorkflowValue::FunctionCall(FunctionCall { name, args }) => {
                if let Some(func) = self.functions.get(name) {
                    let arg_values = args
                        .iter()
                        .map(|arg| Box::new(self.evaluate_param(arg, registers)) as Box<dyn Any + Send>)
                        .collect();
                    let result = func.call(arg_values).await;
                    match result {
                        Ok(result) => {
                            if let Ok(result) = result.downcast::<String>() {
                                (*result).clone()
                            } else {
                                eprintln!("Function call to '{}' did not return a String.", name);
                                "0".to_string()
                            }
                        }
                        Err(err) => {
                            eprintln!("Error executing function '{}': {}", name, err);
                            "0".to_string()
                        }
                    }
                } else {
                    eprintln!("Function '{}' not found.", name);
                    "0".to_string()
                }
            }
            _ => {
                eprintln!("Unsupported workflow value type {:?}, defaulting to 0", value);
                "0".to_string()
            }
        }
    }

    pub fn iter(&'a self, workflow: &'a Workflow) -> StepExecutor<'a> {
        StepExecutor {
            engine: self,
            workflow,
            current_step: 0,
            registers: HashMap::new(),
        }
    }
}

impl<'a> Iterator for StepExecutor<'a> {
    type Item = Result<HashMap<String, String>, WorkflowError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_step < self.workflow.steps.len() {
            let step = &self.workflow.steps[self.current_step];
            for body in &step.body {
                if let Err(e) = futures::executor::block_on(self.engine.execute_step_body(body, &mut self.registers)) {
                    return Some(Err(e));
                }
            }
            self.current_step += 1;
            Some(Ok(self.registers.clone()))
        } else {
            None
        }
    }
}
