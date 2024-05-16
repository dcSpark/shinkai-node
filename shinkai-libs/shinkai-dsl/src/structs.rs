use pest_derive::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[grammar = "workflow.pest"]
pub struct WorkflowParser;

#[derive(Debug, Deserialize)]
pub struct Task {
    pub name: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Statement {
    Task(Task),
    FunctionCall(FunctionCall),
}

#[derive(Debug, Deserialize)]
pub struct Workflow {
    pub workflow: String,
    pub tasks: Vec<Task>,
    pub functions: Vec<Function>,
    pub function_calls: Vec<FunctionCall>,
}