#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::collections::HashMap;

    use shinkai_dsl::{
        dsl_schemas::{Action, ComparisonOperator, Expression, FunctionCall, Param, StepBody, WorkflowValue},
        parser::parse_workflow,
        sm_executor::{WorkflowEngine, WorkflowError},
    };

    #[test]
    fn test_workflow_executor() {
        let dsl_input = r#"
        workflow MyProcess v0.1 {
            step Initialize {
                $R1 = 5
                $R2 = 10
                $R3 = 0
            }
            step Compute {
                if $R1 < $R2 {
                    $R3 = call sum($R2, $R1)
                }
            }
            step Finalize {
                $R3 = call sum($R3, $R1)
            }
        }
        "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");
        println!("workflow: {:?}", workflow);

        // Assert the workflow name and version
        assert_eq!(workflow.name, "MyProcess");
        assert_eq!(workflow.version, "v0.1");

        // Assert the number of steps
        assert_eq!(workflow.steps.len(), 3);

        // Assert details of the Initialize step
        assert_eq!(workflow.steps[0].name, "Initialize");
        assert!(matches!(workflow.steps[0].body[0], StepBody::Composite(ref bodies) if bodies.len() == 3));

        // Assert details of the Compute step
        assert_eq!(workflow.steps[1].name, "Compute");
        assert!(
            matches!(workflow.steps[1].body[0], StepBody::Condition { ref condition, .. } if matches!(condition, Expression::Binary { operator: ComparisonOperator::Less, .. }))
        );

        // Assert details of the Finalize step
        assert_eq!(workflow.steps[2].name, "Finalize");
        assert!(
            matches!(workflow.steps[2].body[0], StepBody::RegisterOperation { ref register, ref value } if register == "$R3" && matches!(value, WorkflowValue::FunctionCall(ref call) if call.name == "sum"))
        );

        // Create function mappings
        let mut functions = HashMap::new();
        functions.insert(
            "sum".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Result<Box<dyn Any>, WorkflowError> {
                let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                Ok(Box::new((x + y).to_string()))
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Result<Box<dyn Any>, WorkflowError> + Send + Sync>,
        );

        eprintln!("\n\n\nStarting workflow execution");
        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .expect("Failed to execute workflow");
        eprintln!("Registers: {:?}", registers);

        // Check the results
        assert_eq!(registers.get("$R1").unwrap(), "5");
        assert_eq!(registers.get("$R2").unwrap(), "10");
        assert_eq!(registers.get("$R3").unwrap(), "20");
    }

    #[test]
    fn test_execute_action_external_function_call() {
        let mut functions = HashMap::new();
        functions.insert(
            "multiply".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Result<Box<dyn Any>, WorkflowError> {
                let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                Ok(Box::new((x * y).to_string()))
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Result<Box<dyn Any>, WorkflowError> + Send + Sync>,
        );

        let executor = WorkflowEngine::new(&functions);
        let mut registers = HashMap::new();
        registers.insert("$R1".to_string(), "5".to_string());
        registers.insert("$R2".to_string(), "10".to_string());

        let action = Action::ExternalFnCall(FunctionCall {
            name: "multiply".to_string(),
            args: vec![
                Param::Identifier("$R1".to_string()),
                Param::Identifier("$R2".to_string()),
            ],
        });

        executor
            .execute_action(&action, &mut registers)
            .expect("Failed to execute action");

        assert_eq!(*registers.get("$R1").unwrap(), "50"); // Assuming the result is stored back in "$R1"
    }

    #[test]
    fn test_evaluate_condition() {
        let functions = HashMap::new();
        let executor = WorkflowEngine::new(&functions);
        let registers = HashMap::from([
            ("$R1".to_string(), "5".to_string()),
            ("$R2".to_string(), "10".to_string()),
        ]);

        let condition = Expression::Binary {
            left: Box::new(Param::Identifier("$R1".to_string())),
            operator: ComparisonOperator::Less,
            right: Box::new(Param::Identifier("$R2".to_string())),
        };

        assert!(executor.evaluate_condition(&condition, &registers).expect("Failed to evaluate condition"));
    }

    #[test]
    fn test_for_loop_execution() {
        let functions = HashMap::new();
        let executor = WorkflowEngine::new(&functions);
        let mut registers = HashMap::new();

        let loop_body = StepBody::RegisterOperation {
            register: "$Sum".to_string(),
            value: WorkflowValue::Identifier("$i".to_string()),
        };

        let for_loop = StepBody::ForLoop {
            var: "$i".to_string(),
            in_expr: Expression::Range {
                start: Box::new(Param::Number(1)),
                end: Box::new(Param::Number(3)),
            },
            action: Box::new(loop_body),
        };

        executor
            .execute_step_body(&for_loop, &mut registers)
            .expect("Failed to execute for loop");

        assert_eq!(registers.get("$Sum").unwrap(), "3"); // Assuming $Sum accumulates values of "$i"
    }

    #[test]
    fn test_step_executor() {
        let dsl_input = r#"
        workflow MyProcess v0.1 {
            step Initialize {
                $R1 = 5
                $R2 = 10
                $R3 = 0
                $R4 = 20
            }
            step Compute {
                if $R1 < $R2 {
                    $R3 = call sum($R2, $R1)
                }
            }
            step Divide {
                $R4 = call divide($R4, $R1)
            }
            step Finalize {
                $R3 = call sum($R3, $R1)
            }
        }
        "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");

        // Create function mappings
        let mut functions = HashMap::new();
        functions.insert(
            "sum".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Result<Box<dyn Any>, WorkflowError> {
                let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                Ok(Box::new((x + y).to_string()))
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Result<Box<dyn Any>, WorkflowError> + Send + Sync>,
        );
        functions.insert(
            "divide".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Result<Box<dyn Any>, WorkflowError> {
                let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
                if y == 0 {
                    Err(WorkflowError::FunctionError("Division by zero".to_string()))
                } else {
                    Ok(Box::new((x / y).to_string()))
                }
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Result<Box<dyn Any>, WorkflowError> + Send + Sync>,
        );

        // Create the WorkflowEngine with the function mappings
        let engine = WorkflowEngine::new(&functions);

        // Create the StepExecutor iterator
        let mut step_executor = engine.iter(&workflow);

        // Execute the workflow step by step
        for (i, result) in step_executor.by_ref().enumerate() {
            let registers = result.expect("Failed to execute step");
            println!("Iteration {}: {:?}", i, registers);
            match i {
                0 => {
                    assert_eq!(registers.get("$R1").unwrap(), "5");
                    assert_eq!(registers.get("$R2").unwrap(), "10");
                    assert_eq!(registers.get("$R3").unwrap(), "0");
                    assert_eq!(registers.get("$R4").unwrap(), "20");
                }
                1 => {
                    assert_eq!(registers.get("$R1").unwrap(), "5");
                    assert_eq!(registers.get("$R2").unwrap(), "10");
                    assert_eq!(registers.get("$R3").unwrap(), "15"); // 10 + 5
                    assert_eq!(registers.get("$R4").unwrap(), "20");
                }
                2 => {
                    assert_eq!(registers.get("$R1").unwrap(), "5");
                    assert_eq!(registers.get("$R2").unwrap(), "10");
                    assert_eq!(registers.get("$R3").unwrap(), "15");
                    assert_eq!(registers.get("$R4").unwrap(), "4"); // 20 / 5
                }
                3 => {
                    assert_eq!(registers.get("$R1").unwrap(), "5");
                    assert_eq!(registers.get("$R2").unwrap(), "10");
                    assert_eq!(registers.get("$R3").unwrap(), "20"); // 15 + 5
                    assert_eq!(registers.get("$R4").unwrap(), "4");
                }
                _ => panic!("Unexpected iteration"),
            }
        }
        // Check the final results
        let final_registers = step_executor.registers;
        assert_eq!(final_registers.get("$R1").unwrap(), "5");
        assert_eq!(final_registers.get("$R2").unwrap(), "10");
        assert_eq!(final_registers.get("$R3").unwrap(), "20");
        assert_eq!(final_registers.get("$R4").unwrap(), "4"); // 20 / 5 = 4
    }

    #[test]
    fn test_string_concatenation() {
        let mut functions = HashMap::new();
        functions.insert(
            "concat".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Result<Box<dyn Any>, WorkflowError> {
                let s1 = args[0].downcast_ref::<String>().unwrap();
                let s2 = args[1].downcast_ref::<String>().unwrap();
                Ok(Box::new(format!("{}{}", s1, s2)))
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Result<Box<dyn Any>, WorkflowError> + Send + Sync>,
        );

        let executor = WorkflowEngine::new(&functions);
        let mut registers = HashMap::new();
        registers.insert("$S1".to_string(), "Hello".to_string());
        registers.insert("$S2".to_string(), "World".to_string());

        let action = Action::ExternalFnCall(FunctionCall {
            name: "concat".to_string(),
            args: vec![
                Param::Identifier("$S1".to_string()),
                Param::Identifier("$S2".to_string()),
            ],
        });

        executor
            .execute_action(&action, &mut registers)
            .expect("Failed to execute action");

        assert_eq!(registers.get("$S1").unwrap(), "HelloWorld"); // Assuming the result is stored back in "$S1"
    }
}
