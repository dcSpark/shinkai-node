use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs::File;
use std::io;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::{any::Any, fmt};

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use futures::Future;
use shinkai_dsl::dsl_schemas::{
    Action, ComparisonOperator, Expression, ForLoopExpression, FunctionCall, Param, StepBody, Workflow, WorkflowValue,
};
use shinkai_sqlite::logger::{WorkflowLogEntry, WorkflowLogEntryStatus};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tokio::task;

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
    logs: Arc<RwLock<VecDeque<WorkflowLogEntry>>>,
}

pub struct StepExecutor<'a> {
    engine: &'a WorkflowEngine<'a>,
    workflow: &'a Workflow,
    pub current_step: usize,
    pub registers: DashMap<String, String>,
    pub logs: Arc<RwLock<VecDeque<WorkflowLogEntry>>>,
}

impl<'a> WorkflowEngine<'a> {
    pub fn new(functions: &'a FunctionMap<'a>) -> Self {
        WorkflowEngine {
            functions,
            logs: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub async fn log(&self, subprocess: Option<String>, additional_info: String, status: WorkflowLogEntryStatus) {
        let mut logs = self.logs.write().await;
        logs.push_back(WorkflowLogEntry {
            subprocess,
            input: None,
            additional_info,
            timestamp: Utc::now(),
            status,
            result: None,
        });
    }

    pub async fn formatted_logs(logs: &RwLock<VecDeque<WorkflowLogEntry>>) -> String {
        let logs = logs.read().await;
        let mut formatted_logs = String::new();
        for log_entry in logs.iter() {
            formatted_logs.push_str(&format!(
                "{}: {}\nStatus: {:?}\n---\n",
                log_entry.subprocess.clone().unwrap_or_default(),
                log_entry.additional_info.replace("\\n", "\n"),
                log_entry.status
            ));
        }
        formatted_logs
    }

    pub async fn write_logs_to_file(logs: &RwLock<VecDeque<WorkflowLogEntry>>, file_path: &str) -> io::Result<()> {
        let logs = logs.read().await;
        let mut file = File::create(file_path)?;

        let now = Utc::now();
        writeln!(file, "Log file created on: {}\n", now.to_rfc2822())?;

        for log_entry in logs.iter() {
            let pretty_value = log_entry.additional_info.replace("\\n", "\n").replace("\\\"", "\"");
            let timestamp = Utc::now().to_rfc3339();
            writeln!(file, "[{}] Subprocess {}: {}\nStatus: {:?}\n---\n", timestamp, log_entry.subprocess.clone().unwrap_or_default(), pretty_value, log_entry.status)?;
        }
        Ok(())
    }

    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        logs: Option<Arc<RwLock<VecDeque<WorkflowLogEntry>>>>,
    ) -> Result<DashMap<String, String>, WorkflowError> {
        let registers = DashMap::new();
        let logs = logs.unwrap_or_else(|| Arc::new(RwLock::new(VecDeque::new())));

        // Log the start of the workflow execution
        {
            let mut logs = logs.write().await;
            logs.push_back(WorkflowLogEntry {
                subprocess: Some("workflow".to_string()),
                input: None,
                additional_info: "Starting workflow execution".to_string(),
                timestamp: Utc::now(),
                status: WorkflowLogEntryStatus::Success("Workflow started".to_string()),
                result: None,
            });
        }

        for step in &workflow.steps {
            for body in &step.body {
                self.execute_step_body(&step.name.clone(), body, &registers, &logs)
                    .await?;
            }
        }
        Ok(registers)
    }

    pub fn execute_step_body<'b>(
        &'b self,
        step_name: &'b str,
        step_body: &'b StepBody,
        registers: &'b DashMap<String, String>,
        logs: &'b Arc<RwLock<VecDeque<WorkflowLogEntry>>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkflowError>> + Send + 'b>> {
        Box::pin(async move {
            match step_body {
                StepBody::Action(action) => {
                    let result = self.execute_action(action, registers).await;
                    {
                        let mut logs = logs.write().await;
                        logs.push_back(WorkflowLogEntry {
                            subprocess: Some(step_name.to_string()),
                            input: None,
                            additional_info: format!("Executing action: {:?}", action),
                            timestamp: Utc::now(),
                            status: match &result {
                                Ok(_) => WorkflowLogEntryStatus::Success("Action executed successfully".to_string()),
                                Err(e) => WorkflowLogEntryStatus::Error(e.to_string()),
                            },
                            result: result.as_ref().ok().map(|_| "Success".to_string()),
                        });
                    }
                    result
                }
                StepBody::Condition { condition, body } => {
                    let condition_result = self.evaluate_condition(condition, registers).await;
                    {
                        let mut logs = logs.write().await;
                        logs.push_back(WorkflowLogEntry {
                            subprocess: Some(step_name.to_string()),
                            input: None,
                            additional_info: format!("Evaluating condition: {:?}, Result: {:?}", condition, condition_result),
                            timestamp: Utc::now(),
                            status: match &condition_result {
                                Ok(_) => WorkflowLogEntryStatus::Success("Condition evaluated successfully".to_string()),
                                Err(e) => WorkflowLogEntryStatus::Error(e.to_string()),
                            },
                            result: condition_result.as_ref().ok().map(|_| "Success".to_string()),
                        });
                    }
                    if condition_result? {
                        self.execute_step_body(step_name, body, registers, logs).await?;
                    }
                    Ok(())
                }
                StepBody::ForLoop { var, in_expr, body } => {
                    match in_expr {
                        ForLoopExpression::Range { start, end } => {
                            let start = self
                                .evaluate_param(start.as_ref(), registers)
                                .await?
                                .parse::<i32>()
                                .unwrap_or(0);
                            let end = self
                                .evaluate_param(end.as_ref(), registers)
                                .await?
                                .parse::<i32>()
                                .unwrap_or(0);
                            for i in start..=end {
                                registers.insert(var.clone(), i.to_string());
                                {
                                    let mut logs = logs.write().await;
                                    logs.push_back(WorkflowLogEntry {
                                        subprocess: Some(step_name.to_string()),
                                        input: Some(format!("{}..{}", start, end)),
                                        additional_info: format!("ForLoop iteration: {} = {}", var, i),
                                        timestamp: Utc::now(),
                                        status: WorkflowLogEntryStatus::Success(format!("Iteration {} completed", i)),
                                        result: Some(format!("Iteration {} completed", i)),
                                    });
                                }
                                self.execute_step_body(step_name, body, registers, logs).await?;
                            }
                        }
                        ForLoopExpression::Split { source, delimiter } => {
                            let source_value = self.evaluate_param(source, registers).await?;
                            let parts: Vec<&str> = source_value.split(delimiter).collect();
                            for part in parts {
                                registers.insert(var.clone(), part.to_string());
                                {
                                    let mut logs = logs.write().await;
                                    logs.push_back(WorkflowLogEntry {
                                        subprocess: Some(step_name.to_string()),
                                        input: Some(source_value.clone()),
                                        additional_info: format!("ForLoop iteration: {} = {}", var, part),
                                        timestamp: Utc::now(),
                                        status: WorkflowLogEntryStatus::Success(format!("Iteration with part '{}' completed", part)),
                                        result: Some(format!("Iteration with part '{}' completed", part)),
                                    });
                                }
                                self.execute_step_body(step_name, body, registers, logs).await?;
                            }
                        }
                    }
                    {
                        let mut logs = logs.write().await;
                        logs.push_back(
                            WorkflowLogEntry {
                                subprocess: Some(step_name.to_string()),
                                input: None,
                                additional_info: format!("Executing for loop: {:?}, Registers: {:?}", in_expr, registers.clone()),
                                timestamp: Utc::now(),
                                status: WorkflowLogEntryStatus::Success("For loop executed successfully".to_string()),
                                result: Some("For loop executed successfully".to_string()),
                            }
                        );
                    }
                    Ok(())
                }
                StepBody::RegisterOperation { register, value } => {
                    let value = self.evaluate_workflow_value(value, registers).await?;
                    registers.insert(register.clone(), value.clone());
                    {
                        let mut logs = logs.write().await;
                        logs.push_back(WorkflowLogEntry {
                            subprocess: Some(step_name.to_string()),
                            input: None,
                            additional_info: format!("Setting register {} to {:?}", register, value),
                            timestamp: Utc::now(),
                            status: WorkflowLogEntryStatus::Success(format!("Register {} set successfully", register)),
                            result: Some(format!("Register {} set successfully", register)),
                        });
                    }
                    Ok(())
                }
                StepBody::Composite(bodies) => {
                    for (index, body) in bodies.iter().enumerate() {
                        let step_body_str = format!("{:?}", body);
                        self.execute_step_body(step_name, body, registers, logs).await?;
                        {
                            let mut logs = logs.write().await;
                            logs.push_back(
                                WorkflowLogEntry {
                                    subprocess: Some(step_name.to_string()),
                                    input: None,
                                    additional_info: format!("Composite body {}: {:?}", index, step_body_str),
                                    timestamp: Utc::now(),
                                    status: WorkflowLogEntryStatus::Success(format!("Composite body {} executed successfully", index)),
                                    result: Some(format!("Composite body {} executed successfully", index)),
                                }
                            );
                        }
                    }
                    Ok(())
                }
            }
        })
    }

    pub async fn execute_action(
        &self,
        action: &Action,
        registers: &DashMap<String, String>,
    ) -> Result<(), WorkflowError> {
        println!("Executing action: {:?}", action);
        match action {
            Action::ExternalFnCall(FunctionCall { name, args }) => {
                println!("Function call: {}", name);
                if let Some(func) = self.functions.get(name) {
                    let arg_values =
                        futures::future::join_all(args.iter().map(|arg| self.evaluate_param(arg, registers))).await;

                    let mut resolved_args = Vec::new();
                    for (i, arg) in arg_values.into_iter().enumerate() {
                        match arg {
                            Ok(value) => {
                                println!("Argument {}: {:?}", i, value);
                                resolved_args.push(Box::new(value) as Box<dyn Any + Send>);
                            }
                            Err(e) => {
                                println!("Failed to evaluate argument {}: {:?}", i, e);
                                return Err(e);
                            }
                        }
                    }

                    // Log the resolved arguments before calling the function
                    for (i, arg) in resolved_args.iter().enumerate() {
                        if let Some(value) = arg.downcast_ref::<String>() {
                            println!("Resolved Argument {}: {:?}", i, value);
                        } else {
                            println!("Resolved Argument {}: Failed to downcast", i);
                        }
                    }

                    eprintln!("Resolved args: {:?}", resolved_args);
                    let result = func.call(resolved_args).await?;
                    if let Some(result) = result.downcast_ref::<String>() {
                        if let Some(Param::Identifier(register_name)) = args.first() {
                            println!("Storing result in register {}: {:?}", register_name, result);
                            registers.insert(register_name.clone(), result.clone());
                        }
                    } else {
                        return Err(WorkflowError::FunctionError("Failed to downcast result".to_string()));
                    }
                } else {
                    return Err(WorkflowError::FunctionError(format!("Function {} not found", name)));
                }
                Ok(())
            }
            _ => Err(WorkflowError::FunctionError(format!("Unhandled action: {:?}", action))),
        }
    }

    pub async fn evaluate_condition(
        &self,
        expression: &Expression,
        registers: &DashMap<String, String>,
    ) -> Result<bool, WorkflowError> {
        match expression {
            Expression::Binary { left, operator, right } => {
                let left_val = self.evaluate_param(left, registers).await?.parse::<i32>().unwrap_or(0);
                let right_val = self.evaluate_param(right, registers).await?.parse::<i32>().unwrap_or(0);
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

    async fn evaluate_param(
        &self,
        param: &Param,
        registers: &DashMap<String, String>,
    ) -> Result<String, WorkflowError> {
        let value = match param {
            Param::String(s) => s.clone(),
            Param::Number(n) => n.to_string(),
            Param::Boolean(b) => b.to_string(),
            Param::Identifier(id) => registers.get(id).map(|v| v.clone()).unwrap_or_default(),
            Param::Register(reg) => registers.get(reg).map(|v| v.clone()).unwrap_or_default(),
            Param::Range(start, end) => format!("{}..{}", start, end),
        };
        Ok(value)
    }

    pub async fn evaluate_workflow_value(
        &self,
        value: &WorkflowValue,
        registers: &DashMap<String, String>,
    ) -> Result<String, WorkflowError> {
        match value {
            WorkflowValue::String(s) => Ok(s.clone()),
            WorkflowValue::Number(n) => Ok(n.to_string()),
            WorkflowValue::Boolean(b) => Ok(b.to_string()),
            WorkflowValue::Identifier(id) | WorkflowValue::Register(id) => registers
                .get(id)
                .map(|v| Ok(v.value().clone()))
                .unwrap_or_else(|| Err(WorkflowError::InvalidArgument(format!("Identifier {} not found", id)))),
            WorkflowValue::FunctionCall(FunctionCall { name, args }) => {
                if let Some(func) = self.functions.get(name) {
                    let mut arg_values = Vec::new();
                    for arg in args {
                        let evaluated_arg = self.evaluate_param(arg, registers).await;
                        // eprintln!("Evaluated arg: {:?}", evaluated_arg);
                        match evaluated_arg {
                            Ok(value) => arg_values.push(Box::new(value) as Box<dyn Any + Send>),
                            Err(e) => {
                                eprintln!("Error evaluating argument: {}", e);
                                return Err(e);
                            }
                        }
                    }

                    let result = func.call(arg_values).await;
                    match result {
                        Ok(result) => {
                            if let Ok(result) = result.downcast::<String>() {
                                Ok((*result).clone())
                            } else {
                                eprintln!("Function call to '{}' did not return a String.", name);
                                Err(WorkflowError::FunctionError(format!(
                                    "Function call to '{}' did not return a String",
                                    name
                                )))
                            }
                        }
                        Err(err) => {
                            eprintln!("Error executing function '{}': {}", name, err);
                            Err(WorkflowError::FunctionError(format!(
                                "Error executing function '{}': {}",
                                name, err
                            )))
                        }
                    }
                } else {
                    eprintln!("Function '{}' not found.", name);
                    Err(WorkflowError::FunctionError(format!("Function '{}' not found", name)))
                }
            }
        }
    }

    pub fn iter(
        &'a self,
        workflow: &'a Workflow,
        initial_registers: Option<DashMap<String, String>>,
        logs: Option<Arc<RwLock<VecDeque<WorkflowLogEntry>>>>,
    ) -> StepExecutor<'a> {
        StepExecutor {
            engine: self,
            workflow,
            current_step: 0,
            registers: initial_registers.unwrap_or_default(),
            logs: logs.unwrap_or_else(|| Arc::new(RwLock::new(VecDeque::new()))),
        }
    }
}

impl<'a> Iterator for StepExecutor<'a> {
    type Item = Result<DashMap<String, String>, WorkflowError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_step < self.workflow.steps.len() {
            let step = &self.workflow.steps[self.current_step];
            let step_name = step.name.clone();

            let mut result = Ok(self.registers.clone());

            task::block_in_place(|| {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    {
                        let mut logs = self.logs.write().await;
                        logs.push_back(WorkflowLogEntry {
                            subprocess: Some(step_name.clone()),
                            input: None,
                            additional_info: format!("Executing step: {:?}", step.name),
                            timestamp: Utc::now(),
                            status: WorkflowLogEntryStatus::Success(format!("Step {} started", step.name)),
                            result: None,
                        });
                    }

                    eprintln!("Executing step: {:?}", step);

                    for body in step.body.iter() {
                        if let Err(e) = self
                            .engine
                            .execute_step_body(&step_name, body, &self.registers, &self.logs)
                            .await
                        {
                            result = Err(e);
                            break;
                        }
                    }
                    if result.is_ok() {
                        result = Ok(self.registers.clone());
                    }
                });
            });

            self.current_step += 1;
            Some(result)
        } else {
            None
        }
    }
}