extern crate pest;
#[macro_use]
extern crate pest_derive;
extern crate serde;
extern crate serde_json;

use pest::Parser;
use shinkai_dsl::structs::{Rule, Workflow};
use shinkai_dsl::{parser::workflow_to_dsl, structs::WorkflowParser};

fn main() {
    let json_input = r#"
    {
        "workflow": "example_workflow",
        "tasks": [
            {
                "name": "task1",
                "dependencies": []
            },
            {
                "name": "task2",
                "dependencies": ["task1"]
            },
            {
                "name": "task3",
                "dependencies": ["task1", "task2"]
            }
        ],
        "functions": [
            {
                "name": "example_function",
                "params": ["param1", "param2"],
                "statements": [
                    {
                        "type": "task",
                        "name": "task4",
                        "dependencies": ["task1"]
                    },
                    {
                        "type": "task",
                        "name": "task5",
                        "dependencies": ["task4"]
                    }
                ]
            }
        ],
        "function_calls": [
            {
                "name": "example_function",
                "args": ["arg1", "arg2"]
            }
        ]
    }
    "#;

    let workflow: Workflow = serde_json::from_str(json_input).expect("Failed to parse JSON");
    eprintln!("{:?}", workflow);

    // Convert JSON to DSL
    let dsl = workflow_to_dsl(&workflow);
    println!("DSL:\n{}", dsl);

    // Print the first character of the DSL
    if let Some(first_char) = dsl.chars().next() {
        println!("First character of DSL: {}", first_char);
    }

    // Parse DSL using Pest
    let parse_result = WorkflowParser::parse(Rule::workflow, &dsl);
    match parse_result {
        Ok(pairs) => {
            for pair in pairs {
                println!("{:?}", pair);
            }
        }
        Err(e) => eprintln!("Failed to parse DSL: {}", e),
    }
}
