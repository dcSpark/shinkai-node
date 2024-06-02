use pest_derive::Parser;
use serde::{Serialize, Deserialize};

#[derive(Parser)]
#[grammar = "workflow.pest"]
pub struct WorkflowParser;

#[derive(Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub version: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub body: Vec<StepBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StepBody {
    Action(Action),
    Condition { condition: Expression, action: Box<StepBody> },
    ForLoop { var: String, in_expr: Expression, action: Box<StepBody> },
    RegisterOperation { register: String, value: WorkflowValue },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    ExternalFnCall(FunctionCall),
    Command { command: String, params: Vec<Param> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Param>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Param {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
    Range(i32, i32),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Expression {
    Binary {
        left: Box<Param>,
        operator: ComparisonOperator,
        right: Box<Param>,
    },
    Range {
        start: Box<Param>,
        end: Box<Param>,
    },
    Simple(Box<Param>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowValue {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
}