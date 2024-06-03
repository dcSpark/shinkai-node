#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::any::Any;

    use shinkai_dsl::{
        dsl_schemas::{Action, ComparisonOperator, Expression, FunctionCall, Param, StepBody, WorkflowValue},
        parser::parse_workflow,
        sm_executor::WorkflowExecutor,
    };

    #[test]
    fn test_workflow_executor() {
        let dsl_input = r#"
        workflow MyProcess v1.0 {
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
        assert_eq!(workflow.version, "v1.0");

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
            Box::new(|args: Vec<Box<dyn Any>>| -> Box<dyn Any> {
                let x = *args[0].downcast_ref::<i32>().unwrap();
                let y = *args[1].downcast_ref::<i32>().unwrap();
                Box::new(x + y)
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Box<dyn Any>>,
        );

        eprintln!("\n\n\nStarting workflow execution");
        // Create the WorkflowExecutor with the function mappings
        let executor = WorkflowExecutor::new(functions);

        // Execute the workflow
        let registers = executor.execute_workflow(&workflow);
        eprintln!("Registers: {:?}", registers);

        // Check the results
        assert_eq!(*registers.get("$R1").unwrap(), 5);
        assert_eq!(*registers.get("$R2").unwrap(), 10);
        assert_eq!(*registers.get("$R3").unwrap(), 20);
    }

    #[test]
    fn test_execute_action_external_function_call() {
        let mut functions = HashMap::new();
        functions.insert(
            "multiply".to_string(),
            Box::new(|args: Vec<Box<dyn Any>>| -> Box<dyn Any> {
                let x = *args[0].downcast_ref::<i32>().unwrap();
                let y = *args[1].downcast_ref::<i32>().unwrap();
                Box::new(x * y)
            }) as Box<dyn Fn(Vec<Box<dyn Any>>) -> Box<dyn Any>>,
        );

        let executor = WorkflowExecutor::new(functions);
        let mut registers = HashMap::new();
        registers.insert("$R1".to_string(), 5);
        registers.insert("$R2".to_string(), 10);

        let action = Action::ExternalFnCall(FunctionCall {
            name: "multiply".to_string(),
            args: vec![Param::Identifier("$R1".to_string()), Param::Identifier("$R2".to_string())],
        });

        executor.execute_action(&action, &mut registers);

        assert_eq!(*registers.get("$R1").unwrap(), 50); // Assuming the result is stored back in "$R1"
    }

    #[test]
    fn test_evaluate_condition() {
        let executor = WorkflowExecutor::new(HashMap::new());
        let registers = HashMap::from([
            ("$R1".to_string(), 5),
            ("$R2".to_string(), 10),
        ]);

        let condition = Expression::Binary {
            left: Box::new(Param::Identifier("$R1".to_string())),
            operator: ComparisonOperator::Less,
            right: Box::new(Param::Identifier("$R2".to_string())),
        };

        assert!(executor.evaluate_condition(&condition, &registers));
    }

    #[test]
    fn test_for_loop_execution() {
        let mut functions = HashMap::new();
        let executor = WorkflowExecutor::new(functions);
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

        executor.execute_step_body(&for_loop, &mut registers);

        assert_eq!(*registers.get("$Sum").unwrap(), 3); // Assuming $Sum accumulates values of "$i"
    }
}
