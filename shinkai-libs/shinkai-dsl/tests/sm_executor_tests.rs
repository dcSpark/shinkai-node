#[cfg(test)]
mod tests {
    use shinkai_dsl::{dsl_schemas::{ComparisonOperator, Expression, StepBody, WorkflowValue}, parser::parse_workflow, sm_executor::WorkflowExecutor};

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

        // Create a mock compute_diff function
        let mut diff_result = 0;
        let compute_diff = |x: i32, y: i32| {
            diff_result = x - y;
        };

        // Create a mock finalize_proc function
        let mut finalized_value = 0;
        let finalize_proc = |x: i32, y: i32| {
            finalized_value = x + y;
        };

        // Create the WorkflowExecutor
        let mut executor = WorkflowExecutor::new(workflow, compute_diff, finalize_proc);

        // Set the initial values of the registers
        executor.registers.insert("R1".to_string(), 5);
        executor.registers.insert("R2".to_string(), 10);
        executor.registers.insert("R3".to_string(), 0);

        // Execute the workflow
        executor.execute();

        // Check the results
        assert_eq!(executor.registers["R1"], 5);
        assert_eq!(executor.registers["R2"], 10);
        assert_eq!(executor.registers["R3"], 20);
        assert_eq!(diff_result, 0); // compute_diff is not called in this example
        assert_eq!(finalized_value, 0); // finalize_proc is not called in this example
    }
}
