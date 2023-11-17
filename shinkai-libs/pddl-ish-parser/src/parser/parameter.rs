#[derive(Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: String,
}

// If you need to parse parameters specifically, you would also implement that logic here.
// For instance, parsing a list of parameters from a PDDL action definition.