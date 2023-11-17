use crate::parser::{action::Action, object::Object};

#[derive(Debug, PartialEq, Clone)]
pub struct Problem {
    pub name: String,
    pub domain: String,
    pub objects: Vec<Object>,
    pub init: Vec<String>,
    pub goal: Vec<String>,
    pub actions: Vec<Action>,
}