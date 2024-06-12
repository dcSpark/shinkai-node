#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::collections::HashMap;

    use async_trait::async_trait;
    use dashmap::DashMap;
    use shinkai_dsl::{
        dsl_schemas::{
            Action, ComparisonOperator, Expression, ForLoopExpression, FunctionCall, Param, StepBody, WorkflowValue,
        },
        parser::parse_workflow,
        sm_executor::{AsyncFunction, FunctionMap, WorkflowEngine, WorkflowError},
    };

    use tokio::time::{sleep, Duration};
    struct SumFunction;

    #[async_trait]
    impl AsyncFunction for SumFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            Ok(Box::new((x + y).to_string()))
        }
    }

    struct MultiplyFunction;

    #[async_trait]
    impl AsyncFunction for MultiplyFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            Ok(Box::new((x * y).to_string()))
        }
    }

    struct DivideFunction;

    #[async_trait]
    impl AsyncFunction for DivideFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            let x = args[0].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            let y = args[1].downcast_ref::<String>().unwrap().parse::<i32>().unwrap();
            if y == 0 {
                Err(WorkflowError::FunctionError("Division by zero".to_string()))
            } else {
                Ok(Box::new((x / y).to_string()))
            }
        }
    }

    struct ConcatFunction;

    #[async_trait]
    impl AsyncFunction for ConcatFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            if args.len() < 2 {
                return Err(WorkflowError::FunctionError(
                    "Not enough arguments for concat".to_string(),
                ));
            }

            let s1 = args[0]
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::FunctionError("Failed to downcast arg[0] to String".to_string()))?;
            let s2 = args[1]
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::FunctionError("Failed to downcast arg[1] to String".to_string()))?;

            Ok(Box::new(format!("{}{}", s1, s2)))
        }
    }

    struct InferenceFunction;

    #[async_trait]
    impl AsyncFunction for InferenceFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            let input = args[0].downcast_ref::<String>().unwrap();
            // Simulate an inference result
            let result = format!("Inference result for: {}", input);
            Ok(Box::new(result))
        }
    }

    struct CloneWithDelayFunction;

    #[async_trait]
    impl AsyncFunction for CloneWithDelayFunction {
        async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
            let input = args[0].downcast_ref::<String>().unwrap().clone();
            let task_response = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;
                input
            })
            .await
            .map_err(|e| WorkflowError::FunctionError(format!("Task failed: {:?}", e)))?;
            Ok(Box::new(task_response))
        }
    }

    #[tokio::test]
    async fn test_workflow_executor() {
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
        functions.insert("sum".to_string(), Box::new(SumFunction) as Box<dyn AsyncFunction>);

        eprintln!("\n\n\nStarting workflow execution");
        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .await
            .expect("Failed to execute workflow");
        eprintln!("Registers: {:?}", registers);

        // Check the results
        assert_eq!(registers.get("$R1").unwrap().as_str(), "5");
        assert_eq!(registers.get("$R2").unwrap().as_str(), "10");
        assert_eq!(registers.get("$R3").unwrap().as_str(), "20");
    }

    #[tokio::test]
    async fn test_execute_action_external_function_call() {
        let mut functions: FunctionMap = HashMap::new();
        functions.insert(
            "multiply".to_string(),
            Box::new(MultiplyFunction) as Box<dyn AsyncFunction>,
        );

        let executor = WorkflowEngine::new(&functions);
        let registers = DashMap::new();
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
            .execute_action(&action, &registers)
            .await
            .expect("Failed to execute action");

        assert_eq!(*registers.get("$R1").unwrap(), "50"); // Assuming the result is stored back in "$R1"
    }

    #[tokio::test]
    async fn test_evaluate_condition() {
        let functions = HashMap::new();
        let executor = WorkflowEngine::new(&functions);
        let registers = DashMap::new();
        registers.insert("$R1".to_string(), "5".to_string());
        registers.insert("$R2".to_string(), "10".to_string());

        let condition = Expression::Binary {
            left: Box::new(Param::Identifier("$R1".to_string())),
            operator: ComparisonOperator::Less,
            right: Box::new(Param::Identifier("$R2".to_string())),
        };

        assert!(executor
            .evaluate_condition(&condition, &registers)
            .await
            .expect("Failed to evaluate condition"));
    }

    #[tokio::test]
    async fn test_for_loop_execution() {
        let functions = HashMap::new();
        let executor = WorkflowEngine::new(&functions);
        let registers = DashMap::new();

        let loop_body = StepBody::RegisterOperation {
            register: "$Last".to_string(),
            value: WorkflowValue::Identifier("$i".to_string()),
        };

        let for_loop = StepBody::ForLoop {
            var: "$i".to_string(),
            in_expr: ForLoopExpression::Range {
                start: Box::new(Param::Number(1)),
                end: Box::new(Param::Number(3)),
            },
            body: Box::new(loop_body),
        };

        executor
            .execute_step_body(&for_loop, &registers)
            .await
            .expect("Failed to execute for loop");

        assert_eq!(registers.get("$Last").unwrap().as_str(), "3"); // Assuming $Sum accumulates values of "$i"
    }

    #[tokio::test]
    async fn test_for_loop_split_sum_execution() {
        let functions = HashMap::new();
        let executor = WorkflowEngine::new(&functions);
        let registers = DashMap::new();

        let loop_body = StepBody::RegisterOperation {
            register: "$Last".to_string(),
            value: WorkflowValue::Identifier("$item".to_string()),
        };

        let for_loop = StepBody::ForLoop {
            var: "$item".to_string(),
            in_expr: ForLoopExpression::Split {
                source: Param::String("1,2,3".to_string()),
                delimiter: ",".to_string(),
            },
            body: Box::new(loop_body),
        };

        executor
            .execute_step_body(&for_loop, &registers)
            .await
            .expect("Failed to execute for loop");

        assert_eq!(registers.get("$Last").unwrap().as_str(), "3"); // Assuming $Sum accumulates values of "$item"
    }

    #[test]
    fn test_step_executor() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
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
            let mut functions: FunctionMap = HashMap::new();
            functions.insert("sum".to_string(), Box::new(SumFunction) as Box<dyn AsyncFunction>);
            functions.insert("divide".to_string(), Box::new(DivideFunction) as Box<dyn AsyncFunction>);

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
                        assert_eq!(registers.get("$R1").unwrap().as_str(), "5");
                        assert_eq!(registers.get("$R2").unwrap().as_str(), "10");
                        assert_eq!(registers.get("$R3").unwrap().as_str(), "0");
                        assert_eq!(registers.get("$R4").unwrap().as_str(), "20");
                    }
                    1 => {
                        assert_eq!(registers.get("$R1").unwrap().as_str(), "5");
                        assert_eq!(registers.get("$R2").unwrap().as_str(), "10");
                        assert_eq!(registers.get("$R3").unwrap().as_str(), "15"); // 10 + 5
                        assert_eq!(registers.get("$R4").unwrap().as_str(), "20");
                    }
                    2 => {
                        assert_eq!(registers.get("$R1").unwrap().as_str(), "5");
                        assert_eq!(registers.get("$R2").unwrap().as_str(), "10");
                        assert_eq!(registers.get("$R3").unwrap().as_str(), "15");
                        assert_eq!(registers.get("$R4").unwrap().as_str(), "4");
                        // 20 / 5
                    }
                    3 => {
                        assert_eq!(registers.get("$R1").unwrap().as_str(), "5");
                        assert_eq!(registers.get("$R2").unwrap().as_str(), "10");
                        assert_eq!(registers.get("$R3").unwrap().as_str(), "20"); // 15 + 5
                        assert_eq!(registers.get("$R4").unwrap().as_str(), "4");
                    }
                    _ => panic!("Unexpected iteration"),
                }
            }
            // Check the final results
            let final_registers = step_executor.registers;
            assert_eq!(final_registers.get("$R1").unwrap().as_str(), "5");
            assert_eq!(final_registers.get("$R2").unwrap().as_str(), "10");
            assert_eq!(final_registers.get("$R3").unwrap().as_str(), "20");
            assert_eq!(final_registers.get("$R4").unwrap().as_str(), "4"); // 20 / 5 = 4
        });
    }

    #[tokio::test]
    async fn test_string_concatenation() {
        let mut functions: FunctionMap = HashMap::new();
        functions.insert("concat".to_string(), Box::new(ConcatFunction) as Box<dyn AsyncFunction>);

        let executor = WorkflowEngine::new(&functions);
        let registers = DashMap::new();
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
            .execute_action(&action, &registers)
            .await
            .expect("Failed to execute action");

        assert_eq!(registers.get("$S1").unwrap().as_str(), "HelloWorld"); // Assuming the result is stored back in "$S1"
    }

    #[tokio::test]
    async fn test_inference_workflow() {
        let dsl_input = r#"
        workflow MyProcess v0.1 {
            step Initialize {
                $R1 = ""
                $R2 = "Tell me about the Economy of the Roman Empire"
            }
            step Inference {
                $R1 = call inference($R2)
            }
        }
        "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");

        // Create function mappings
        let mut functions: FunctionMap = HashMap::new();
        functions.insert(
            "inference".to_string(),
            Box::new(InferenceFunction) as Box<dyn AsyncFunction>,
        );

        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .await
            .expect("Failed to execute workflow");

        // Check the results
        assert_eq!(
            registers.get("$R1").unwrap().as_str(),
            "Inference result for: Tell me about the Economy of the Roman Empire"
        );
        assert_eq!(
            registers.get("$R2").unwrap().as_str(),
            "Tell me about the Economy of the Roman Empire"
        );
    }

    #[tokio::test]
    async fn test_clone_workflow() {
        let dsl_input = r#"
        workflow MyProcess v0.1 {
            step Initialize {
                $R1 = ""
                $R2 = "Clone this string"
            }
            step Clone {
                $R1 = call clone($R2)
            }
        }
        "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");

        // Create function mappings
        let mut functions: FunctionMap = HashMap::new();
        functions.insert(
            "clone".to_string(),
            Box::new(CloneWithDelayFunction) as Box<dyn AsyncFunction>,
        );

        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .await
            .expect("Failed to execute workflow");

        // Check the results
        assert_eq!(registers.get("$R1").unwrap().as_str(), "Clone this string");
        assert_eq!(registers.get("$R2").unwrap().as_str(), "Clone this string");
    }

    #[tokio::test]
    async fn test_for_loop_with_split() {
        let dsl_input = r#"
    workflow MyProcess v0.1 {
        step Initialize {
            $R1 = "red,blue,green"
            $R2 = ""
        }
        step SplitAndIterate {
            for item in $R1.split(",") {
                $R2 = call concat($R2, item)
            }
            $R1 = $R2
        }
    }
    "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");
        eprintln!("workflow: {:?}", workflow);

        // Create function mappings
        let mut functions: FunctionMap = HashMap::new();
        functions.insert("concat".to_string(), Box::new(ConcatFunction) as Box<dyn AsyncFunction>);

        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .await
            .expect("Failed to execute workflow");

        // Check the results
        assert_eq!(registers.get("$R1").unwrap().as_str(), "red,blue,green");
        assert_eq!(registers.get("$R2").unwrap().as_str(), "redbluegreen");
    }

    #[tokio::test]
    async fn test_register_assignment() {
        let dsl_input = r#"
        workflow MyProcess v0.1 {
            step Initialize {
                $R1 = "red,blue,green"
                $R2 = $R1
            }
        }
        "#;

        let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");

        // Create function mappings
        let functions: FunctionMap = HashMap::new();

        // Create the WorkflowEngine with the function mappings
        let executor = WorkflowEngine::new(&functions);

        // Execute the workflow
        let registers = executor
            .execute_workflow(&workflow)
            .await
            .expect("Failed to execute workflow");

        // Check the results
        assert_eq!(registers.get("$R1").unwrap().as_str(), "red,blue,green");
        assert_eq!(registers.get("$R2").unwrap().as_str(), "red,blue,green");
    }
}
