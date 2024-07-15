#[cfg(test)]
mod tests {
    use pest::Parser;
    use shinkai_dsl::{
        dsl_schemas::{
            Action, ComparisonOperator, Expression, ForLoopExpression, Param, Rule, StepBody, WorkflowParser,
            WorkflowValue,
        },
        parser::{parse_action, parse_expression, parse_step, parse_step_body, parse_step_body_item, parse_workflow},
    };

    #[test]
    fn test_parse_workflow() {
        let input = r#"workflow myWorkflow v1.0 { step myStep { command("param1", "param2") } }"#;
        let result = parse_workflow(input);
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.name, "myWorkflow");
        assert_eq!(workflow.version, "v1.0");
        assert_eq!(workflow.steps.len(), 1);
    }

    #[test]
    fn test_parse_step() {
        let input = r#"step myStep { command("param1", "param2") }"#;
        let pair = WorkflowParser::parse(Rule::step, input).unwrap().next().unwrap();
        let result = parse_step(pair);
        assert!(result.is_ok());
        let step = result.unwrap();
        assert_eq!(step.name, "myStep");
        assert_eq!(step.body.len(), 1);
    }

    #[test]
    fn test_parse_step_with_for_loop() {
        let input = r#"step myStep { for var in 0..10 { command("doSomething") } }"#;
        let pair = WorkflowParser::parse(Rule::step, input).unwrap().next().unwrap();
        let result = parse_step(pair);
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        let step = result.unwrap();
        assert_eq!(step.name, "myStep");
        assert_eq!(step.body.len(), 1);

        match &step.body[0] {
            StepBody::ForLoop { var, in_expr, body } => {
                assert_eq!(var, "var");
                match in_expr {
                    ForLoopExpression::Range { start, end } => {
                        match **start {
                            Param::Number(start_value) => assert_eq!(start_value, 0),
                            _ => panic!("Expected Number as start of range"),
                        }
                        match **end {
                            Param::Number(end_value) => assert_eq!(end_value, 10),
                            _ => panic!("Expected Number as end of range"),
                        }
                    }
                    _ => panic!("Expected Range expression in ForLoop"),
                }
                match **body {
                    StepBody::Action(Action::Command {
                        ref command,
                        ref params,
                    }) => {
                        assert_eq!(command, "command");
                        assert_eq!(params.len(), 1);
                        match &params[0] {
                            Param::String(ref s) => assert_eq!(s, "doSomething"),
                            _ => panic!("Expected String parameter"),
                        }
                    }
                    _ => panic!("Expected Command action within ForLoop"),
                }
            }
            _ => panic!("Expected ForLoop in step body"),
        }
    }

    #[test]
    fn test_parse_action_command() {
        let input = r#"command("param1", "param2")"#;
        let pair = WorkflowParser::parse(Rule::action, input).unwrap().next().unwrap();
        let action = parse_action(pair);
        match action {
            Action::Command { command, params } => {
                assert_eq!(command, "command");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected Command action"),
        }
    }

    #[test]
    fn test_parse_action_external_fn_call() {
        let input = r#"call functionName("param1", "param2")"#;
        let pair = WorkflowParser::parse(Rule::action, input).unwrap().next().unwrap();
        let action = parse_action(pair);
        match action {
            Action::ExternalFnCall(fn_call) => {
                assert_eq!(fn_call.name, "functionName");
                assert_eq!(fn_call.args.len(), 2);
            }
            _ => panic!("Expected ExternalFnCall action"),
        }
    }

    #[test]
    fn test_parse_expression() {
        let input = r#"param1 == "value""#;
        let pair = WorkflowParser::parse(Rule::expression, input).unwrap().next().unwrap();
        let expression = parse_expression(pair);

        match expression {
            Expression::Binary { left, operator, right } => {
                assert_eq!(operator, ComparisonOperator::Equal);
                match *left {
                    Param::Identifier(ref id) => assert_eq!(id, "param1"),
                    _ => panic!("Expected Identifier on left side"),
                }
                match *right {
                    Param::String(ref s) => assert_eq!(s, "value"),
                    _ => panic!("Expected String on right side"),
                }
            }
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_parse_range_expression() {
        let input = r#"0..10"#;
        let pair = WorkflowParser::parse(Rule::range_expression, input)
            .unwrap()
            .next()
            .unwrap();
        let expression = parse_expression(pair);

        match expression {
            Expression::Range { start, end } => {
                match *start {
                    Param::Number(start_value) => assert_eq!(start_value, 0),
                    _ => panic!("Expected Number as start of range"),
                }
                match *end {
                    Param::Number(end_value) => assert_eq!(end_value, 10),
                    _ => panic!("Expected Number as end of range"),
                }
            }
            _ => panic!("Expected Range expression"),
        }
    }

    #[test]
    fn test_parse_condition() {
        let input = r#"if param1 > 10 { command("doSomething") }"#;
        let pair = WorkflowParser::parse(Rule::step_body, input).unwrap().next().unwrap();
        eprintln!("{:?}", pair);
        let step_body = parse_step_body(pair);
        println!("{:?}", step_body);
        match step_body {
            StepBody::Condition {
                condition,
                body: action,
            } => {
                match condition {
                    Expression::Binary { left, operator, right } => {
                        assert_eq!(operator, ComparisonOperator::Greater);
                        match *left {
                            Param::Identifier(ref id) => assert_eq!(id, "param1"),
                            _ => panic!("Expected Identifier on left side"),
                        }
                        match *right {
                            Param::Number(num) => assert_eq!(num, 10),
                            _ => panic!("Expected Number on right side"),
                        }
                    }
                    _ => panic!("Expected Binary Expression with an operator"),
                }
                match *action {
                    StepBody::Action(_) => (),
                    _ => panic!("Expected Action within Condition"),
                }
            }
            _ => panic!("Expected Condition"),
        }
    }

    #[test]
    fn test_parse_register_operation() {
        let input = r#"$R1 = 42"#;
        let pair = WorkflowParser::parse(Rule::register_operation, input)
            .unwrap()
            .next()
            .unwrap();
        let step_body = parse_step_body_item(pair);
        match step_body {
            StepBody::RegisterOperation { register, value } => {
                assert_eq!(register, "$R1");
                match value {
                    WorkflowValue::Number(num) => assert_eq!(num, 42),
                    _ => panic!("Expected Number value"),
                }
            }
            _ => panic!("Expected RegisterOperation"),
        }
    }

    #[test]
    fn test_parse_workflow_multiple_steps() {
        let input = r#"workflow complexWorkflow v2.0 {
            step stepOne { command("paramA", "paramB") }
            step stepTwo { for i in 1..5 { command("loopCommand") } }
            step stepThree { if paramX > 10 { command("conditionalCommand") } }
        }
    "#;
        let result = parse_workflow(input);
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.name, "complexWorkflow");
        assert_eq!(workflow.version, "v2.0");
        assert_eq!(workflow.steps.len(), 3);

        // Check first step
        let step_one = &workflow.steps[0];
        assert_eq!(step_one.name, "stepOne");
        assert_eq!(step_one.body.len(), 1);

        // Check second step
        let step_two = &workflow.steps[1];
        assert_eq!(step_two.name, "stepTwo");
        assert_eq!(step_two.body.len(), 1);
        match step_two.body.first().unwrap() {
            StepBody::ForLoop { var, in_expr, body } => {
                assert_eq!(var, "i");
                match in_expr {
                    ForLoopExpression::Range { start, end } => {
                        match **start {
                            Param::Number(start_value) => assert_eq!(start_value, 1),
                            _ => panic!("Expected Number as start of range"),
                        }
                        match **end {
                            Param::Number(end_value) => assert_eq!(end_value, 5),
                            _ => panic!("Expected Number as end of range"),
                        }
                    }
                    _ => panic!("Expected Range expression in ForLoop"),
                }
                match **body {
                    StepBody::Action(_) => (),
                    _ => panic!("Expected Action within ForLoop"),
                }
            }
            _ => panic!("Expected ForLoop"),
        }

        // Check third step
        let step_three = &workflow.steps[2];
        assert_eq!(step_three.name, "stepThree");
        assert_eq!(step_three.body.len(), 1);
        match step_three.body.first().unwrap() {
            StepBody::Condition {
                condition,
                body: action,
            } => {
                match condition {
                    Expression::Binary { left, operator, right } => {
                        assert_eq!(*operator, ComparisonOperator::Greater);
                        match **left {
                            Param::Identifier(ref id) => assert_eq!(id, "paramX"),
                            _ => panic!("Expected Identifier on left side"),
                        }
                        match **right {
                            Param::Number(num) => assert_eq!(num, 10),
                            _ => panic!("Expected Number on right side"),
                        }
                    }
                    _ => panic!("Expected Binary Expression with an operator"),
                }
                match **action {
                    StepBody::Action(_) => (),
                    _ => panic!("Expected Action within Condition"),
                }
            }
            _ => panic!("Expected Condition"),
        }
    }
}
