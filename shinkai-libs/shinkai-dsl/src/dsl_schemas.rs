use pest_derive::Parser;
use serde::{Deserialize, Serialize};

use crate::parser::parse_workflow;

#[derive(Parser)]
#[grammar = "workflow.pest"]
pub struct WorkflowParser;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub name: String,
    pub version: String,
    pub steps: Vec<Step>,
    pub raw: String,
    pub description: Option<String>,
    pub author: String,
    pub sticky: bool,
}

impl Workflow {
    /// Generates a key for the Workflow using its name and version.
    pub fn generate_key(&self) -> String {
        format!("{}:::{}", self.name, self.version)
    }

    /// Creates a Workflow from a JSON string and a description.
    pub fn new(
        dsl_input: String,
        description: String,
    ) -> Result<Self, String> {
        let workflow = parse_workflow(&dsl_input)?;
        Ok(Workflow {
            description: Some(description),
            author: workflow.author,
            sticky: workflow.sticky,
            ..workflow
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub body: Vec<StepBody>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    ExternalFnCall(FunctionCall),
    Command { command: String, params: Vec<Param> },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Param>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum Param {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
    Range(i32, i32),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ForLoopExpression {
    Split { source: Param, delimiter: String },
    Range { start: Box<Param>, end: Box<Param> },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum WorkflowValue {
    String(String),
    Number(i64),
    Boolean(bool),
    Identifier(String),
    Register(String),
    FunctionCall(FunctionCall),
}
