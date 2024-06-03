#[cfg(test)]
mod workflow_tests {
    use pest::Parser;
    use workflow_input_parser::{Rule, WorkflowParser};

    mod workflow_input_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        workflow  = { "workflow" ~ identifier ~ version ~ "{" ~ step+ ~ "}" }
        step      = { "step" ~ identifier ~ "{" ~ step_body ~ "}" }
        step_body = { (condition | register_operation | action | for_loop )+ }
        condition = { "if" ~ expression ~ "{" ~ step_body ~ "}" }
        for_loop  = { "for" ~ identifier ~ "in" ~ expression ~ "{" ~ action ~ "}" }
        action    = { external_fn_call | command ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        command   = { identifier }
        param     = { string | number | boolean | identifier | register }
        register  = { "$" ~ "R" ~ ASCII_DIGIT }
        // New rule for registers
        external_fn_call   = { "call" ~ identifier ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        expression         = { range_expression | simple_expression ~ (comparison_operator ~ simple_expression)? }
        simple_expression  = { identifier | number | boolean | string | register }
        range_expression   = { identifier ~ ".." ~ identifier }
        register_operation = { register ~ "=" ~ (value | external_fn_call) }
        // New rule for register operations
        comparison_operator =  { "==" | "!=" | ">" | "<" | ">=" | "<=" }
        value               =  { string | number | boolean | identifier | register }
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
    fn test_register_operations() {
        let workflow_str = r#"workflow MyProcess v1.0 {
            step Initialize {
                $R1 = 5
                $R2 = 10
            }
            step Compute {
                if $R1 < $R2 {
                    call compute_difference($R2, $R1)
                }
            }
            step Finalize {
                call finalize_process($R1, $R2)
            }
        }"#;

        let pairs = WorkflowParser::parse(Rule::workflow, workflow_str).unwrap();
        eprintln!("{:?}", pairs);

        for pair in pairs.into_iter() {
            match pair.as_rule() {
                Rule::workflow => {
                    let mut inner_pairs = pair.into_inner();
                    assert_eq!(inner_pairs.next().unwrap().as_str(), "MyProcess");
                    assert_eq!(inner_pairs.next().unwrap().as_str().trim(), "v1.0");

                    for inner_pair in inner_pairs {
                        match inner_pair.as_rule() {
                            Rule::step => {
                                let mut step_inner_pairs = inner_pair.into_inner();
                                let step_identifier = step_inner_pairs.next().unwrap().as_str();
                                let step_body = step_inner_pairs.next().unwrap();

                                for step_body_part in step_body.into_inner() {
                                    match step_body_part.as_rule() {
                                        Rule::register_operation => {
                                            let mut parts = step_body_part.into_inner();
                                            let register = parts.next().unwrap().as_str();
                                            let value = parts.next().unwrap().as_str().trim(); // Trim the value here
                                            assert!(register == "$R1" || register == "$R2");
                                            assert!(value == "5" || value == "10");
                                        }
                                        Rule::condition => {
                                            let condition_inner = step_body_part.into_inner().next().unwrap();
                                            let expression_parts =
                                                condition_inner.into_inner().map(|p| p.as_str()).collect::<Vec<_>>();
                                            assert_eq!(expression_parts, ["$R1", "<", "$R2"]);
                                        }
                                        Rule::action => {
                                            let action_inner = step_body_part.into_inner().next().unwrap();
                                            let action_identifier = action_inner.into_inner().next().unwrap().as_str();
                                            assert!(
                                                action_identifier == "compute_difference"
                                                    || action_identifier == "finalize_process"
                                            );
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
