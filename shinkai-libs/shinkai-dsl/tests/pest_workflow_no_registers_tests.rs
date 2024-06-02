#[cfg(test)]
mod workflow_tests {
    use pest::Parser;

    mod workflow_no_input_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        workflow            =  { "workflow" ~ identifier ~ version ~ "{" ~ step+ ~ "}" }
        step                =  { "step" ~ identifier ~ "{" ~ step_body ~ "}" }
        step_body           =  { condition ~ action ~ "}" | action | for_loop }
        condition           =  { "if" ~ expression ~ "{" }
        for_loop            =  { "for" ~ identifier ~ "in" ~ expression ~ "{" ~ action ~ "}" }
        action              =  { external_fn_call | command ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        command             =  { identifier }
        param               =  { string | number | boolean | identifier }
        external_fn_call    =  { "call" ~ identifier ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        expression          =  { identifier ~ comparison_operator ~ value | range_expression }
        range_expression    =  { identifier ~ ".." ~ identifier }
        comparison_operator =  { "==" | "!=" | ">" | "<" | ">=" | "<=" }
        value               =  { string | number | boolean | identifier }
        version             =  { "v" ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)* }
        identifier          = @{ (ASCII_ALPHANUMERIC | "_")+ }
        string              = _{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
        number              = _{ ASCII_DIGIT+ }
        boolean             =  { "true" | "false" }
        WHITESPACE          = _{ " " | "\t" | "\n" | "\r" }
        "#]
        pub struct WorkflowParser;
    }

    #[test]
    fn test_workflow() {
        use workflow_no_input_parser::Rule;
        use workflow_no_input_parser::WorkflowParser;

        let workflow_str = r#"workflow MyProcess v1.0 {
            step Step1 {
                if hola == hello {
                    call some_function(param1, param2)
                }
            }
            step Step2 {
                call another_function(param3)
            }
        }"#;
        let pairs = WorkflowParser::parse(Rule::workflow, workflow_str).unwrap();
        eprintln!("{:?}", pairs);

        // Assuming the outermost rule is workflow, we need to handle it first
        for pair in pairs.into_iter() {
            match pair.as_rule() {
                Rule::workflow => {
                    let mut inner_pairs = pair.into_inner();
                    let identifier = inner_pairs.next().unwrap();
                    assert_eq!(identifier.as_str(), "MyProcess");
                    let version = inner_pairs.next().unwrap();
                    assert_eq!(version.as_str().trim(), "v1.0");

                    for inner_pair in inner_pairs {
                        match inner_pair.as_rule() {
                            Rule::step => {
                                let mut step_inner_pairs = inner_pair.into_inner();
                                let step_identifier = step_inner_pairs.next().unwrap().as_str();
                                let step_body = step_inner_pairs.next().unwrap();
                                let step_body_inner = step_body.into_inner().next().unwrap();

                                match step_body_inner.as_rule() {
                                    Rule::condition => {
                                        let condition_inner = step_body_inner.into_inner().next().unwrap();
                                        let expression = condition_inner.into_inner();
                                        let expression_parts = expression.map(|p| p.as_str()).collect::<Vec<_>>();
                                        assert_eq!(expression_parts, ["hola", "==", "hello"]);
                                    }
                                    Rule::action => {
                                        let action_inner = step_body_inner.into_inner().next().unwrap();
                                        let action_identifier = action_inner.into_inner().next().unwrap().as_str();
                                        if step_identifier == "Step1" {
                                            assert_eq!(action_identifier, "some_function");
                                        } else if step_identifier == "Step2" {
                                            assert_eq!(action_identifier, "another_function");
                                        }
                                    }
                                    _ => panic!("Unexpected rule in step body"),
                                }
                            }
                            _ => panic!("Unexpected rule; expected a step"),
                        }
                    }
                }
                _ => panic!("Unexpected rule; expected a workflow"),
            }
        }
    }

    #[test]
    fn test_for_loop_parsing() {
        use workflow_no_input_parser::Rule;
        use workflow_no_input_parser::WorkflowParser;

        let workflow_str = r#"workflow MyProcess v1.0 {
        step Step1 {
            for item in 1..10 {
                call process_item(item)
            }
        }
        step Step2 {
            call finalize_process()
        }
    }"#;

        let pairs = WorkflowParser::parse(Rule::workflow, workflow_str).unwrap();
        eprintln!("{:?}", pairs);

        for pair in pairs.into_iter() {
            println!("Rule: {:?}, Content: '{}'", pair.as_rule(), pair.as_str()); // Print the top-level rule and its content
            match pair.as_rule() {
                Rule::workflow => {
                    let mut inner_pairs = pair.into_inner();
                    let identifier = inner_pairs.next().expect("Expected identifier in workflow");
                    println!("Identifier: '{}'", identifier.as_str()); // Print the identifier
                    assert_eq!(identifier.as_str(), "MyProcess");

                    let version = inner_pairs.next().expect("Expected version in workflow");
                    println!("Version: '{}'", version.as_str().trim()); // Print the version
                    assert_eq!(version.as_str().trim(), "v1.0");

                    for inner_pair in inner_pairs {
                        println!(
                            "Step Rule: {:?}, Content: '{}'",
                            inner_pair.as_rule(),
                            inner_pair.as_str()
                        ); // Print each step
                        match inner_pair.as_rule() {
                            Rule::step => {
                                let mut step_inner_pairs = inner_pair.into_inner();
                                let step_identifier = step_inner_pairs.next().expect("Expected step identifier");
                                println!("Step Identifier: '{}'", step_identifier.as_str()); // Print the step identifier

                                let step_body = step_inner_pairs.next().expect("Expected step body");
                                for step_body_part in step_body.into_inner() {
                                    println!(
                                        "Step Body Part Rule: {:?}, Content: '{}'",
                                        step_body_part.as_rule(),
                                        step_body_part.as_str()
                                    ); // Print each part of the step body
                                    match step_body_part.as_rule() {
                                        Rule::for_loop => {
                                            let mut loop_inner = step_body_part.into_inner();
                                            let loop_variable =
                                                loop_inner.next().expect("Expected loop variable").as_str();
                                            println!("Loop Variable: '{}'", loop_variable); // Print the loop variable

                                            let loop_range_expression =
                                                loop_inner.next().expect("Expected loop range expression");
                                            let loop_range = loop_range_expression
                                                .into_inner()
                                                .map(|p| p.as_str())
                                                .collect::<Vec<_>>();
                                            println!("Loop Range: {:?}", loop_range); // Print the loop range

                                            let action_inner = loop_inner.next().expect("Expected action in loop");
                                            let action_identifier = action_inner
                                                .into_inner()
                                                .next()
                                                .expect("Expected action identifier")
                                                .as_str();
                                            println!("Action Identifier: '{}'", action_identifier);
                                            // Print the action identifier
                                        }
                                        Rule::action => {
                                            let action_inner =
                                                step_body_part.into_inner().next().expect("Expected action");
                                            match action_inner.as_rule() {
                                                Rule::external_fn_call => {
                                                    let mut action_parts = action_inner.into_inner();
                                                    let command =
                                                        action_parts.next().expect("Expected command").as_str();
                                                    println!("Command: '{}'", command); // Print the command

                                                    let params = action_parts.map(|p| p.as_str()).collect::<Vec<_>>();
                                                    println!("Params: {:?}", params);
                                                    // Print the parameters
                                                }
                                                _ => panic!("Unexpected rule in action: {:?}", action_inner.as_rule()),
                                            }
                                        }
                                        _ => panic!("Unexpected rule in step body: {:?}", step_body_part.as_rule()),
                                    }
                                }
                            }
                            _ => panic!("Unexpected rule; expected a step"),
                        }
                    }
                }
                _ => panic!("Unexpected rule; expected a workflow"),
            }
        }
    }
}
