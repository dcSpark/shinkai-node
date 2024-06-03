use std::collections::HashMap;
use std::any::Any;

use crate::dsl_schemas::{Action, ComparisonOperator, Expression, FunctionCall, Param, StepBody, Workflow, WorkflowValue};

pub struct WorkflowExecutor {
    functions: HashMap<String, Box<dyn Fn(Vec<Box<dyn Any>>) -> Box<dyn Any>>>,
}

impl WorkflowExecutor {
    pub fn new(functions: HashMap<String, Box<dyn Fn(Vec<Box<dyn Any>>) -> Box<dyn Any>>>) -> Self {
        WorkflowExecutor {
            functions,
        }
    }

    pub fn execute_workflow(&self, workflow: &Workflow) -> HashMap<String, i32> {
        let mut registers = HashMap::new();
        for step in &workflow.steps {
            for body in &step.body {
                self.execute_step_body(body, &mut registers);
            }
        }
        registers
    }

    pub fn execute_step_body(&self, step_body: &StepBody, registers: &mut HashMap<String, i32>) {
        match step_body {
            StepBody::Action(action) => {
                self.execute_action(action, registers);
            },
            StepBody::Condition { condition, body } => {
                if self.evaluate_condition(condition, registers) {
                    self.execute_step_body(body, registers);
                }
            },
            StepBody::ForLoop { var, in_expr, action } => {
                if let Expression::Range { start, end } = in_expr {
                    let start = self.evaluate_param(start.as_ref(), registers);
                    let end = self.evaluate_param(end.as_ref(), registers);
                    for i in start..=end {
                        registers.insert(var.clone(), i);
                        self.execute_step_body(action, registers);
                    }
                }
            },
            StepBody::RegisterOperation { register, value } => {
                println!("Setting register {} to {:?}", register, value);
                let value = self.evaluate_workflow_value(value, registers);
                println!("Value: {}", value);
                registers.insert(register.clone(), value);
            },
            StepBody::Composite(bodies) => {
                for body in bodies {
                    self.execute_step_body(body, registers);
                }
            },
        }
    }

    pub fn execute_action(&self, action: &Action, registers: &mut HashMap<String, i32>) {
        println!("Executing action: {:?}", action);
        match action {
            Action::ExternalFnCall(FunctionCall { name, args }) => {
                if let Some(func) = self.functions.get(name) {
                    let arg_values = args.iter().map(|arg| Box::new(self.evaluate_param(arg, registers)) as Box<dyn Any>).collect();
                    let result = func(arg_values);
                    if let Ok(result) = result.downcast::<i32>() {
                        // Assuming the result should be stored in a specific register, which should be defined in your DSL or function call semantics
                        // For example, if the result register is always the first argument:
                        if let Some(Param::Identifier(register_name)) = args.first() {
                            registers.insert(register_name.clone(), *result);
                        }
                    }
                }
            },
            _ => {
                // Handle other action types
                println!("Unhandled action: {:?}", action);
            }
        }
    }

    pub fn evaluate_condition(&self, expression: &Expression, registers: &HashMap<String, i32>) -> bool {
        match expression {
            Expression::Binary { left, operator, right } => {
                let left_val = self.evaluate_param(left, registers);
                let right_val = self.evaluate_param(right, registers);
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

    pub fn evaluate_param(&self, param: &Param, registers: &HashMap<String, i32>) -> i32 {
        match param {
            Param::Number(n) => *n as i32,
            Param::Identifier(id) | Param::Register(id) => {
                registers.get(id).copied().unwrap_or_else(|| {
                    eprintln!("Warning: Identifier/Register '{}' not found in registers, defaulting to 0", id);
                    0
                })
            },
            _ => {
                eprintln!("Warning: Unsupported parameter type, defaulting to 0");
                0
            },
        }
    }

    pub fn evaluate_workflow_value(&self, value: &WorkflowValue, registers: &HashMap<String, i32>) -> i32 {
        match value {
            WorkflowValue::Number(n) => *n as i32,
            WorkflowValue::Identifier(id) => *registers.get(id).unwrap_or(&0),
            WorkflowValue::FunctionCall(FunctionCall { name, args }) => {
                if let Some(func) = self.functions.get(name) {
                    let arg_values = args.iter().map(|arg| Box::new(self.evaluate_param(arg, registers)) as Box<dyn Any>).collect();
                    let result = func(arg_values);
                    if let Ok(result) = result.downcast::<i32>() {
                        *result
                    } else {
                        eprintln!("Function call to '{}' did not return an i32.", name);
                        0
                    }
                } else {
                    eprintln!("Function '{}' not found.", name);
                    0
                }
            },
            _ => {
                eprintln!("Unsupported workflow value type {:?}, defaulting to 0", value);
                0
            },
        }
    }
}