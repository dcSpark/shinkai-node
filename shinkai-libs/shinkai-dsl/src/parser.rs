use pest::Parser;

use crate::dsl_schemas::{
    Action, ComparisonOperator, Expression, ForLoopExpression, FunctionCall, Param, Rule, Step, StepBody, Workflow,
    WorkflowParser, WorkflowValue,
};

pub fn parse_step_body(pair: pest::iterators::Pair<Rule>) -> StepBody {
    if pair.as_rule() != Rule::step_body {
        panic!("Expected 'step_body' rule, found {:?}", pair.as_rule());
    }

    let inner_pairs = pair.into_inner().peekable();
    let mut bodies = Vec::new();

    for inner_pair in inner_pairs {
        bodies.push(parse_step_body_item(inner_pair));
    }

    if bodies.len() == 1 {
        bodies.remove(0)
    } else {
        StepBody::Composite(bodies) // Assuming there is a Composite variant to handle multiple bodies
    }
}

pub fn parse_step_body_item(pair: pest::iterators::Pair<Rule>) -> StepBody {
    match pair.as_rule() {
        Rule::action => {
            StepBody::Action(parse_action(pair))
        }
        Rule::condition => {
            let mut inner_pairs = pair.into_inner();
            let expression = parse_expression(inner_pairs.next().expect("Expected expression in condition"));
            let body = parse_step_body(inner_pairs.next().expect("Expected step body in condition"));

            StepBody::Condition {
                condition: expression,
                body: Box::new(body),
            }
        }
        Rule::for_loop => {
            let mut loop_inner_pairs = pair.into_inner();
            let var_pair = loop_inner_pairs.next().expect("Expected variable in for loop");
            let in_expr_pair = loop_inner_pairs.next().expect("Expected expression in for loop");
            let body_pair = loop_inner_pairs.next().expect("Expected action in for loop");

            let in_expr = match in_expr_pair.as_rule() {
                Rule::split_expression => {
                    let mut split_inner_pairs = in_expr_pair.into_inner();
                    let source = parse_param(split_inner_pairs.next().expect("Expected source in split expression"));
                    
                    let delimiter_pair = split_inner_pairs.next().expect("Expected delimiter in split expression");
                    let delimiter = delimiter_pair.as_str().trim_matches('"').to_string();
                    
                    ForLoopExpression::Split { source, delimiter }
                }
                Rule::range_expression => ForLoopExpression::Range {
                    start: Box::new(parse_param(
                        in_expr_pair
                            .clone()
                            .into_inner()
                            .next()
                            .expect("Expected start of range"),
                    )),
                    end: Box::new(parse_param(
                        in_expr_pair.clone().into_inner().nth(1).expect("Expected end of range"),
                    )),
                },
                _ => panic!("Unexpected rule in for loop expression: {:?}", in_expr_pair.as_rule()),
            };

            StepBody::ForLoop {
                var: var_pair.as_str().to_string(),
                in_expr,
                body: Box::new(parse_step_body(body_pair)),
            }
        }
        Rule::register_operation => {
            let mut register_inner_pairs = pair.into_inner();
            let register_pair = register_inner_pairs
                .next()
                .expect("Expected register in register operation");
            let value_pair = register_inner_pairs
                .next()
                .expect("Expected value in register operation");
            StepBody::RegisterOperation {
                register: register_pair.as_str().trim().to_string(),
                value: parse_workflow_value(value_pair),
            }
        }
        _ => panic!("Unexpected rule in step body item: {:?}", pair.as_rule()),
    }
}

pub fn parse_value_or_call(pair: pest::iterators::Pair<Rule>) -> WorkflowValue {
    match pair.as_rule() {
        Rule::value => parse_workflow_value(pair),
        Rule::external_fn_call => WorkflowValue::FunctionCall(parse_external_fn_call(pair)),
        _ => panic!("Expected value or external function call, found {:?}", pair.as_rule()),
    }
}

pub fn parse_external_fn_call(pair: pest::iterators::Pair<Rule>) -> FunctionCall {
    let mut inner_pairs = pair.into_inner();
    let name_pair = inner_pairs
        .next()
        .expect("Expected function name in external function call");
    let args = inner_pairs.map(parse_param).collect();

    FunctionCall {
        name: name_pair.as_str().to_string(),
        args,
    }
}

pub fn parse_action(pair: pest::iterators::Pair<Rule>) -> Action {
    let mut inner_pairs = pair.into_inner();
    let first_pair = inner_pairs.next().expect("Expected content in action");

    match first_pair.as_rule() {
        Rule::external_fn_call => {
            let mut fn_call_inner_pairs = first_pair.into_inner();
            let name_pair = fn_call_inner_pairs
                .next()
                .expect("Expected function name in external function call");
            let args = fn_call_inner_pairs.map(parse_param).collect();

            Action::ExternalFnCall(FunctionCall {
                name: name_pair.as_str().to_string(),
                args,
            })
        }
        Rule::command => {
            let command = first_pair.as_str().to_string();
            let params = inner_pairs
                .map(|p| {
                    // Assuming each 'param' pair directly contains the parameter as its only inner pair
                    // let actual_param = p.into_inner().next().expect("Expected parameter content");
                    // eprintln!("Actual param: {:?}", actual_param.as_rule());
                    parse_param(p)
                })
                .collect::<Vec<_>>();

            Action::Command { command, params }
        }
        _ => panic!("Unexpected rule in action: {:?}", first_pair.as_rule()),
    }
}

pub fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Expression {
    match pair.as_rule() {
        Rule::range_expression => {
            let mut inner_pairs = pair.into_inner();
            let start = parse_param(inner_pairs.next().expect("Expected start of range"));
            let end = parse_param(inner_pairs.next().expect("Expected end of range"));
            Expression::Range {
                start: Box::new(start),
                end: Box::new(end),
            }
        }
        Rule::expression => {
            let mut inner_pairs = pair.into_inner();
            let first_expr = parse_param(
                inner_pairs
                    .next()
                    .expect("Expected first expression in simple expression"),
            );

            if let Some(operator_pair) = inner_pairs.next() {
                let operator = parse_comparison_operator(operator_pair);
                let second_expr = parse_param(
                    inner_pairs
                        .next()
                        .expect("Expected second expression in simple expression"),
                );
                Expression::Binary {
                    left: Box::new(first_expr),
                    operator,
                    right: Box::new(second_expr),
                }
            } else {
                // If there's no operator, it means the expression is just a simple expression
                // Assuming `first_expr` can be directly used as an Expression
                Expression::Simple(Box::new(first_expr))
            }
        }
        _ => panic!("Unexpected expression type: {:?}", pair.as_rule()),
    }
}

pub fn parse_param(pair: pest::iterators::Pair<Rule>) -> Param {
    let input = pair.as_str().trim();

    match identify_param_type(input) {
        "string" => {
            // Remove the surrounding quotes from the string value
            let stripped_string = input.trim_matches('"').to_string();
            Param::String(stripped_string)
        }
        "number" => {
            // Parse the string as a number, assuming it's valid since it matched the number rule
            let number = input.parse().expect("Failed to parse number");
            Param::Number(number)
        }
        "boolean" => {
            // Parse the string as a boolean, assuming it's valid since it matched the boolean rule
            let boolean = input.parse().expect("Failed to parse boolean");
            Param::Boolean(boolean)
        }
        "register" => {
            // Directly use the string as a register
            Param::Register(input.to_string())
        }
        "identifier" => {
            // Directly use the string as an identifier
            Param::Identifier(input.to_string())
        }
        "range" => {
            let parts: Vec<&str> = input.split("..").collect();
            if parts.len() == 2 {
                let start = parts[0].parse().expect("Failed to parse start of range");
                let end = parts[1].parse().expect("Failed to parse end of range");
                Param::Range(start, end)
            } else {
                panic!("Invalid range expression: {}", input);
            }
        }
        _ => panic!("Unexpected parameter type: {}", input),
    }
}

fn identify_param_type(input: &str) -> &str {
    let input = input.trim();
    if input.starts_with('"') && input.ends_with('"') {
        "string"
    } else if input.contains("..") {
        "range"
    } else if input.parse::<f64>().is_ok() {
        "number"
    } else if input == "true" || input == "false" {
        "boolean"
    } else if input.starts_with('$') && input[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        "register"
    } else if input.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        "identifier"
    } else {
        "unknown"
    }
}

pub fn parse_comparison_operator(pair: pest::iterators::Pair<Rule>) -> ComparisonOperator {
    match pair.as_str() {
        "==" => ComparisonOperator::Equal,
        "!=" => ComparisonOperator::NotEqual,
        ">" => ComparisonOperator::Greater,
        "<" => ComparisonOperator::Less,
        ">=" => ComparisonOperator::GreaterEqual,
        "<=" => ComparisonOperator::LessEqual,
        _ => panic!("Unexpected comparison operator: {}", pair.as_str()),
    }
}

pub fn parse_workflow_value(pair: pest::iterators::Pair<Rule>) -> WorkflowValue {
    let input = pair.as_str().trim(); // Trim leading and trailing spaces

    match pair.as_rule() {
        Rule::value => {
            // Directly parse the value based on its content
            if input.starts_with('"') && input.ends_with('"') {
                let stripped_string = input.trim_matches('"').to_string();
                WorkflowValue::String(stripped_string)
            } else if input.parse::<i64>().is_ok() {
                let number = input.parse::<i64>().expect("Failed to parse number");
                WorkflowValue::Number(number)
            } else if input == "true" || input == "false" {
                let boolean = input.parse::<bool>().expect("Failed to parse boolean");
                WorkflowValue::Boolean(boolean)
            } else if input.starts_with('$') {
                WorkflowValue::Register(input.to_string())
            } else {
                WorkflowValue::Identifier(input.to_string())
            }
        }
        Rule::external_fn_call => WorkflowValue::FunctionCall(parse_external_fn_call(pair)),
        _ => panic!("Unexpected rule in parse_workflow_value: {:?}", pair.as_rule()),
    }
}

pub fn parse_workflow(dsl_input: &str) -> Result<Workflow, String> {
    let trimmed_input = dsl_input.trim_start(); // Remove leading spaces and newlines
    let pairs = WorkflowParser::parse(Rule::workflow, trimmed_input).map_err(|e| e.to_string())?;

    let mut workflow_name = String::new();
    let mut version = String::new();
    let mut steps = Vec::new();
    let mut author = "@@not_defined.shinkai".to_string(); // Default value for author
    let mut sticky = false; // Default value for sticky

    for pair in pairs {
        match pair.as_rule() {
            Rule::workflow => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::identifier => {
                            workflow_name = inner_pair.as_str().to_string();
                        }
                        Rule::version => {
                            version = inner_pair.as_str().trim().to_string();
                        }
                        Rule::step => {
                            steps.push(parse_step(inner_pair)?);
                        }
                        Rule::identity => {
                            let identity = inner_pair.as_str().to_string();
                            author = format!("@@{}", identity);
                        }
                        Rule::sticky_tag => {
                            sticky = true;
                        }
                        _ => return Err("Unexpected rule in workflow parsing".to_string()),
                    }
                }
            }
            _ => return Err("Top level rule must be workflow".to_string()),
        }
    }

    Ok(Workflow {
        name: workflow_name,
        version,
        steps,
        raw: dsl_input.to_string(),
        description: None,
        author,
        sticky,
    })
}

pub fn parse_step(pair: pest::iterators::Pair<Rule>) -> Result<Step, String> {
    let mut step_name = String::new();
    let mut bodies = Vec::new();

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::identifier => {
                step_name = inner_pair.as_str().to_string();
            }
            Rule::step_body => {
                // Assuming step_body can directly contain action, condition, etc.
                bodies.push(parse_step_body(inner_pair));
            }
            _ => return Err("Unexpected rule in step parsing".to_string()),
        }
    }

    Ok(Step {
        name: step_name,
        body: bodies,
    })
}
