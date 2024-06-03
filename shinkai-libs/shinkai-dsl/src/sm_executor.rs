use std::collections::HashMap;

use crate::dsl_schemas::Action;
use crate::dsl_schemas::ComparisonOperator;
use crate::dsl_schemas::Expression;
use crate::dsl_schemas::FunctionCall;
use crate::dsl_schemas::Param;
use crate::dsl_schemas::StepBody;
use crate::dsl_schemas::Workflow;

pub struct WorkflowExecutor<F, G>
where
    F: FnMut(i32, i32),
    G: FnMut(i32, i32),
{
    pub registers: HashMap<String, i32>,
    pub workflow: Workflow,
    pub compute_diff: Option<F>,
    pub finalize_proc: G,
}

impl<F, G> WorkflowExecutor<F, G>
where
    F: FnMut(i32, i32),
    G: FnMut(i32, i32),
{
    pub fn new(workflow: Workflow, compute_diff: F, finalize_proc: G) -> Self {
        WorkflowExecutor {
            registers: HashMap::new(),
            workflow,
            compute_diff: Some(compute_diff),
            finalize_proc,
        }
    }

    pub fn execute(&mut self) {
        let actions_to_execute = self.collect_actions();

        let mut compute_diff = self.compute_diff.take().expect("compute_diff was not set");

        for action in actions_to_execute {
            WorkflowExecutor::<F, G>::execute_action(&action, &mut compute_diff, &self.registers);
        }

        self.compute_diff = Some(compute_diff);
    }

    fn collect_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for step in &self.workflow.steps {
            for body in &step.body {
                match body {
                    StepBody::Action(action) => {
                        actions.push((*action).clone());
                    },
                    StepBody::Condition { condition, body: action } => {
                        if self.evaluate_condition(condition) {
                            match **action {
                                StepBody::Action(ref action) => {
                                    actions.push((*action).clone());
                                },
                                _ => {}
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
        actions
    }

    fn evaluate_condition(&self, expression: &Expression) -> bool {
        match expression {
            Expression::Binary { left, operator, right } => {
                let left_val = self.evaluate_param(left);
                let right_val = self.evaluate_param(right);
                match operator {
                    ComparisonOperator::Less => left_val < right_val,
                    ComparisonOperator::Greater => left_val > right_val,
                    ComparisonOperator::Equal => left_val == right_val,
                    ComparisonOperator::NotEqual => left_val != right_val,
                    ComparisonOperator::LessEqual => left_val <= right_val,
                    ComparisonOperator::GreaterEqual => left_val >= right_val,
                }
            }
            _ => false,
        }
    }

    fn evaluate_param(&self, param: &Param) -> i32 {
        match param {
            Param::Number(n) => *n as i32,
            Param::Identifier(id) => *self.registers.get(id).unwrap_or(&0),
            _ => 0,
        }
    }

    // Now a static method
    fn execute_action(action: &Action, func: &mut F, registers: &HashMap<String, i32>) {
        match action {
            Action::ExternalFnCall(FunctionCall { name, args }) => {
                if args.len() == 2 {
                    let arg1 = WorkflowExecutor::<F, G>::evaluate_param_static(&args[0], registers);
                    let arg2 = WorkflowExecutor::<F, G>::evaluate_param_static(&args[1], registers);
                    func(arg1, arg2);
                }
            },
            Action::Command { command, params } => {
                println!("Executing command: {}, with params: {:?}", command, params);
            },
            _ => {}
        }
    }

    // Helper static method for parameter evaluation
    fn evaluate_param_static(param: &Param, registers: &HashMap<String, i32>) -> i32 {
        match param {
            Param::Number(n) => *n as i32,
            Param::Identifier(id) => *registers.get(id).unwrap_or(&0),
            _ => 0,
        }
    }
}