use crate::structs::{Function, FunctionCall, Rule, Statement, Task, Workflow, WorkflowParser};
use pest::Parser;

pub fn workflow_to_dsl(workflow: &Workflow) -> String {
    let mut dsl = String::new();
    dsl.push_str(&format!("workflow {} {{\n", workflow.workflow));

    for task in &workflow.tasks {
        dsl.push_str(&format!("    task \"{}\" {{\n", task.name));
        if !task.dependencies.is_empty() {
            dsl.push_str("        depends_on [");
            dsl.push_str(
                &task
                    .dependencies
                    .iter()
                    .map(|d| format!("\"{}\"", d))
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            dsl.push_str("]\n");
        }
        dsl.push_str("    }\n");
    }

    for function in &workflow.functions {
        dsl.push_str(&format!("    function {}(", function.name));
        dsl.push_str(&function.params.join(", "));
        dsl.push_str(") {\n");
        for statement in &function.statements {
            match statement {
                Statement::Task(task) => {
                    dsl.push_str(&format!("        task \"{}\" {{\n", task.name));
                    if !task.dependencies.is_empty() {
                        dsl.push_str("            depends_on [");
                        dsl.push_str(
                            &task
                                .dependencies
                                .iter()
                                .map(|d| format!("\"{}\"", d))
                                .collect::<Vec<String>>()
                                .join(", "),
                        );
                        dsl.push_str("]\n");
                    }
                    dsl.push_str("        }\n");
                }
                Statement::FunctionCall(call) => {
                    dsl.push_str(&format!("        {}(", call.name));
                    dsl.push_str(
                        &call
                            .args
                            .iter()
                            .map(|a| format!("\"{}\"", a))
                            .collect::<Vec<String>>()
                            .join(", "),
                    );
                    dsl.push_str(")\n");
                }
            }
        }
        dsl.push_str("    }\n");
    }

    for call in &workflow.function_calls {
        dsl.push_str(&format!("    {}(", call.name));
        dsl.push_str(
            &call
                .args
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<String>>()
                .join(", "),
        );
        dsl.push_str(")\n");
    }

    dsl.push_str("}\n");
    dsl
}

pub fn parse_dsl(dsl: &str) -> Result<Workflow, pest::error::Error<Rule>> {
    let pairs = WorkflowParser::parse(Rule::workflow, dsl)?;
    let mut workflow = Workflow {
        workflow: String::new(),
        tasks: Vec::new(),
        functions: Vec::new(),
        function_calls: Vec::new(),
    };

    for pair in pairs {
        match pair.as_rule() {
            Rule::workflow => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::string => {
                            workflow.workflow = inner_pair.as_str().trim_matches('"').to_string();
                        }
                        Rule::task => {
                            let mut task = Task {
                                name: String::new(),
                                dependencies: Vec::new(),
                            };
                            for task_pair in inner_pair.into_inner() {
                                match task_pair.as_rule() {
                                    Rule::string => {
                                        task.name = task_pair.as_str().trim_matches('"').to_string();
                                    }
                                    Rule::depends_on => {
                                        for dep in task_pair.into_inner() {
                                            task.dependencies.push(dep.as_str().trim_matches('"').to_string());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            workflow.tasks.push(task);
                        }
                        Rule::function => {
                            let mut function = Function {
                                name: String::new(),
                                params: Vec::new(),
                                statements: Vec::new(),
                            };
                            for func_pair in inner_pair.into_inner() {
                                match func_pair.as_rule() {
                                    Rule::ident => {
                                        function.name = func_pair.as_str().to_string();
                                    }
                                    Rule::statement => {
                                        for stmt_pair in func_pair.into_inner() {
                                            match stmt_pair.as_rule() {
                                                Rule::task => {
                                                    let mut task = Task {
                                                        name: String::new(),
                                                        dependencies: Vec::new(),
                                                    };
                                                    for task_pair in stmt_pair.into_inner() {
                                                        match task_pair.as_rule() {
                                                            Rule::string => {
                                                                task.name =
                                                                    task_pair.as_str().trim_matches('"').to_string();
                                                            }
                                                            Rule::depends_on => {
                                                                for dep in task_pair.into_inner() {
                                                                    task.dependencies.push(
                                                                        dep.as_str().trim_matches('"').to_string(),
                                                                    );
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    function.statements.push(Statement::Task(task));
                                                }
                                                Rule::function_call => {
                                                    let mut call = FunctionCall {
                                                        name: String::new(),
                                                        args: Vec::new(),
                                                    };
                                                    for call_pair in stmt_pair.into_inner() {
                                                        match call_pair.as_rule() {
                                                            Rule::ident => {
                                                                call.name = call_pair.as_str().to_string();
                                                            }
                                                            Rule::string => {
                                                                call.args.push(
                                                                    call_pair.as_str().trim_matches('"').to_string(),
                                                                );
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    function.statements.push(Statement::FunctionCall(call));
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            workflow.functions.push(function);
                        }
                        Rule::function_call => {
                            let mut call = FunctionCall {
                                name: String::new(),
                                args: Vec::new(),
                            };
                            for call_pair in inner_pair.into_inner() {
                                match call_pair.as_rule() {
                                    Rule::ident => {
                                        call.name = call_pair.as_str().to_string();
                                    }
                                    Rule::string => {
                                        call.args.push(call_pair.as_str().trim_matches('"').to_string());
                                    }
                                    _ => {}
                                }
                            }
                            workflow.function_calls.push(call);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(workflow)
}
