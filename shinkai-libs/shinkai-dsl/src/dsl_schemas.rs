use pest_derive::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[grammar = "workflow.pest"]
pub struct WorkflowParser;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workflow {
    pub name: String,
    pub version: String,
    pub steps: Vec<Step>,
    pub raw: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Step {
    pub name: String,
    pub body: Vec<StepBody>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StepBody {
    Action(Action),
    Condition {
        condition: Expression,
        body: Box<StepBody>,
    },
    ForLoop {
        var: String,
        in_expr: ForLoopExpression,
        body: Box<StepBody>,
    },
    RegisterOperation {
        register: String,
        value: WorkflowValue,
    },
    Composite(Vec<StepBody>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    ExternalFnCall(FunctionCall),
    Command { command: String, params: Vec<Param> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Param>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Param {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
    Range(i32, i32),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ForLoopExpression {
    Split { source: Param, delimiter: String },
    Range { start: Box<Param>, end: Box<Param> },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum WorkflowValue {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
    FunctionCall(FunctionCall),
}
