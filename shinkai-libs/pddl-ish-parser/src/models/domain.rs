use crate::parser::{action::Action, predicate::Predicate, domain_type::DomainType};

#[derive(Debug, PartialEq, Clone)]
pub struct Domain {
    pub name: String,
    pub requirements: Vec<String>,
    pub types: Vec<DomainType>,
    pub predicates: Vec<Predicate>,
    pub actions: Vec<Action>,
}
