#[cfg(test)]
mod workflow_tests {
    use pest::Parser;

    mod workflow_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        workflow            =  { "workflow" ~ identifier ~ version ~ "{" ~ step+ ~ "}" }
        step                =  { "step" ~ identifier ~ "{" ~ step_body ~ "}" }
        step_body           =  { condition ~ action ~ "}" | action }
        condition           =  { "if" ~ expression ~ "{" }
        action              =  { external_fn_call | command ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        command             =  { identifier }
        param               =  { string | number | boolean | identifier }
        external_fn_call    =  { "call" ~ identifier ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        expression          =  { identifier ~ comparison_operator ~ value }
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
        use workflow_parser::Rule;
        use workflow_parser::WorkflowParser;

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
}
