#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::any::Any;

    use shinkai_dsl::{
        dsl_schemas::{ComparisonOperator, Expression, StepBody, WorkflowValue},
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
}
