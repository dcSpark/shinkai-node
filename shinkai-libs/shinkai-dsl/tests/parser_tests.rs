#[cfg(test)]
mod tests {
    use pest::Parser;
    use shinkai_dsl::{
        dsl_schemas::{
            Action, ComparisonOperator, Expression, ForLoopExpression, Param, Rule, StepBody, Workflow, WorkflowParser,
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

    #[test]
    fn test_parse_serialize_deserialize_workflow() {
        let input = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;
        let result = parse_workflow(input);
        assert!(result.is_ok());
        let workflow = result.unwrap();

        // Serialize the workflow
        let serialized_workflow = serde_json::to_string(&workflow).expect("Failed to serialize workflow");

        // Deserialize the workflow
        let deserialized_workflow: Workflow =
            serde_json::from_str(&serialized_workflow).expect("Failed to deserialize workflow");

        // Deep comparison
        assert_eq!(workflow, deserialized_workflow);
    }

    #[test]
    fn test_get_agility_story_workflow() {
        pub const AGILITY_STORY_SYSTEM: &str = r#"
        # IDENTITY and PURPOSE

        You are an expert in the Agile framework. You deeply understand user story and acceptance criteria creation. You will be given a topic. Please write the appropriate information for what is requested. 

        # STEPS

        Please write a user story and acceptance criteria for the requested topic.

        # OUTPUT INSTRUCTIONS

        Output the results in JSON format as defined in this example:

        {
            "Topic": "Automating data quality automation",
            "Story": "As a user, I want to be able to create a new user account so that I can access the system.",
            "Criteria": "Given that I am a user, when I click the 'Create Account' button, then I should be prompted to enter my email address, password, and confirm password. When I click the 'Submit' button, then I should be redirected to the login page."
        }

        # INPUT:

        INPUT:
        "#;

        let agility_escaped = AGILITY_STORY_SYSTEM.replace('"', "\\\"");

        let raw_workflow = format!(
            r#"
                workflow Agility_story v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}
            "#,
            agility_escaped
        );
        // eprintln!("raw_workflow: {}\n\n", raw_workflow);

        let result = parse_workflow(&raw_workflow);
        assert!(result.is_ok());
        let mut workflow = result.unwrap();
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        assert_eq!(workflow.name, "Agility_story");
        assert_eq!(workflow.version, "v0.1");
        assert_eq!(workflow.steps.len(), 1);
        assert_eq!(
            workflow.description,
            Some("Generates workflow based on the provided system.md.".to_string())
        );

        let step = &workflow.steps[0];
        assert_eq!(step.name, "Main");
        assert_eq!(step.body.len(), 1);

        match &step.body[0] {
            StepBody::Composite(composite_body) => {
                assert_eq!(composite_body.len(), 2);

                match &composite_body[0] {
                    StepBody::RegisterOperation { register, value } => {
                        assert_eq!(register, "$SYSTEM");
                        match value {
                            WorkflowValue::String(system_value) => assert_eq!(*system_value, agility_escaped),
                            _ => panic!("Expected String value for $SYSTEM"),
                        }
                    }
                    _ => panic!("Expected RegisterOperation for $SYSTEM"),
                }

                match &composite_body[1] {
                    StepBody::RegisterOperation { register, value } => {
                        assert_eq!(register, "$RESULT");
                        match value {
                            WorkflowValue::FunctionCall(fn_call) => {
                                assert_eq!(fn_call.name, "opinionated_inference");
                                assert_eq!(fn_call.args.len(), 2);
                                match &fn_call.args[0] {
                                    Param::Register(input) => assert_eq!(input, "$INPUT"),
                                    _ => panic!("Expected Register for $INPUT"),
                                }
                                match &fn_call.args[1] {
                                    Param::Register(system) => assert_eq!(system, "$SYSTEM"),
                                    _ => panic!("Expected Register for $SYSTEM"),
                                }
                            }
                            _ => panic!("Expected FunctionCall value for $RESULT"),
                        }
                    }
                    _ => panic!("Expected RegisterOperation for $RESULT"),
                }
            }
            _ => panic!("Expected Composite in step body"),
        }
    }

    #[test]
    fn test_parse_workflow_with_author_and_sticky() {
        let input = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            } @@nico.arb-sep-shinkai sticky
        "#;
        let result = parse_workflow(input);
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        let workflow = result.unwrap();

        assert_eq!(workflow.name, "ExtensiveSummary");
        assert_eq!(workflow.version, "v0.1");
        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.author, "@@nico.arb-sep-shinkai".to_string());
        assert!(workflow.sticky);
    }

    #[test]
    fn test_parse_workflow_with_author_no_sticky() {
        let input = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            } @@nico.arb-sep-shinkai
        "#;
        let result = parse_workflow(input);
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        let workflow = result.unwrap();

        assert_eq!(workflow.name, "ExtensiveSummary");
        assert_eq!(workflow.version, "v0.1");
        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.author, "@@nico.arb-sep-shinkai".to_string());
        assert!(!workflow.sticky);
    }

    #[test]
    fn test_parse_workflow_with_sticky_no_author() {
        let input = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            } sticky
        "#;
        let result = parse_workflow(input);
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        let workflow = result.unwrap();

        assert_eq!(workflow.name, "ExtensiveSummary");
        assert_eq!(workflow.version, "v0.1");
        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.author, "@@not_defined.shinkai".to_string());
        assert!(workflow.sticky);
    }

    #[test]
    fn test_extract_function_names() {
        let input = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call shinkai__weather_by_city($PROMPT, $EMBEDDINGS)
                }
            } @@nico.arb-sep-shinkai
        "#;
        let result = parse_workflow(input);
        assert!(result.is_ok());
        let workflow = result.unwrap();

        let function_names = workflow.extract_function_names();
        assert_eq!(
            function_names,
            vec!["process_embeddings_in_job_scope", "shinkai__weather_by_city"]
        );
    }

    #[test]
    fn test_parse_workflow_with_comments() {
        let input = r##"
            # This is a comment
            # Another comment line
            workflow commentWorkflow v1.0 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                    # Inline comment
                    $LLM_INPUT = call generate_json_map("question", $INPUT, "documents", $FILE_PIECES)
# Inline comment                    
                    $LLM_RESPONSE = call baml_answer_with_citations($LLM_INPUT)
                    $JINJA = "# Introduction\n{%- for sentence in answer.brief_introduction.sentences %}\n{{ sentence }}\n{%- endfor %}\n\n# Main Content\n{%- if answer.extensive_body | length > 1 %}\n{%- for part in answer.extensive_body %}\n### Part {{ loop.index }}\n{%- for sentence in part.sentences %}\n{{ sentence }}\n{%- endfor %}\n{%- endfor %}\n{%- else %}\n{%- for part in answer.extensive_body %}\n{%- for sentence in part.sentences %}\n{{ sentence }}\n{%- endfor %}\n{%- endfor %}\n{%- endif %}\n\n# Conclusion\n{%- for section in answer.conclusion %}\n{%- for sentence in section.sentences %}\n{{ sentence }}\n{%- endfor %}\n{%- endfor %}\n\n# Citations\n{%- for citation in relevantSentencesFromText %}\n[{{ citation.citation_id }}]: {{ citation.relevantTextFromDocument }} ({{ citation.document_reference }})\n{%- endfor %}"
                    $RESULT = call shinkai__json-to-md("message", $LLM_RESPONSE, "template", $JINJA)
                }
            } @@official.shinkai
        "##;
        let result = parse_workflow(input);
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.name, "commentWorkflow");
        assert_eq!(workflow.version, "v1.0");
        assert_eq!(workflow.steps.len(), 1);

        let step = &workflow.steps[0];
        assert_eq!(step.name, "Initialize");
        let function_names = workflow.extract_function_names();
        assert_eq!(
            function_names,
            vec![
                "process_embeddings_in_job_scope",
                "generate_json_map",
                "baml_answer_with_citations",
                "shinkai__json-to-md"
            ]
        );
    }
}
