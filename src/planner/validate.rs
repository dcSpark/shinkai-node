use super::runner::Plan;
use serde_json::json;
use serde_json::Result;
use std::collections::HashMap;

impl Plan {
    pub fn validate_plan(plan_json: &str) -> Result<Vec<String>> {
        let plan: Plan = serde_json::from_str(plan_json)?;
        let mut errors = Vec::new();
        let mut task_ids = Vec::new();
        let mut reachable = HashMap::new();

        // First, populate task_ids
        for task in &plan.tasks {
            // Check if task_id is empty
            if task.task_id.is_empty() {
                errors.push(format!("Task ID is empty for task: {:?}", task));
            }

            // Check if task_id is unique
            if task_ids.contains(&task.task_id) {
                errors.push(format!("Task ID '{}' is not unique.", task.task_id));
            } else {
                task_ids.push(task.task_id.clone());
            }
        }

        // Then, perform the checks for success.next and failure.next
        for task in &plan.tasks {
            // Check if success.next and failure.next refer to an existing task
            if !task.success.next.is_empty() && !task_ids.contains(&task.success.next) {
                errors.push(format!(
                    "'success.next' '{}' does not refer to an existing task.",
                    task.success.next
                ));
            }
            if !task.failure.next.is_empty() && !task_ids.contains(&task.failure.next) {
                errors.push(format!(
                    "'failure.next' '{}' does not refer to an existing task.",
                    task.failure.next
                ));
            }

            // Check if process_inputs refer to existent task_inputs
            for &index in &task.process_inputs {
                if index >= task.task_inputs.len() {
                    errors.push(format!(
                        "'process_inputs' index '{}' is out of bounds for task_inputs in task '{}'.",
                        index, task.task_id
                    ));
                }
            }

            // Add to reachable map
            reachable.entry(task.task_id.clone()).or_insert(vec![]);
            if !task.success.next.is_empty() {
                reachable
                    .entry(task.task_id.clone())
                    .or_insert(vec![])
                    .push(task.success.next.clone());
            }
            if !task.failure.next.is_empty() {
                reachable
                    .entry(task.task_id.clone())
                    .or_insert(vec![])
                    .push(task.failure.next.clone());
            }
        }

        // Perform DFS to check if all tasks are reachable
        let mut visited = HashMap::new();
        for task_id in &task_ids {
            visited.insert(task_id.clone(), false);
        }
        Self::dfs(&plan.states.initial, &mut visited, &reachable);

        for (task_id, &is_visited) in &visited {
            if !is_visited {
                errors.push(format!("Task '{}' is not reachable from the initial task.", task_id));
            }
        }

        Ok(errors)
    }

    fn dfs(node: &String, visited: &mut HashMap<String, bool>, reachable: &HashMap<String, Vec<String>>) {
        visited.insert(node.clone(), true);
        if let Some(neighbors) = reachable.get(node) {
            for neighbor in neighbors {
                if let Some(is_visited) = visited.get(neighbor) {
                    if !is_visited {
                        Self::dfs(neighbor, visited, reachable);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plan_empty_task_id() {
        let plan_json = r#"
        {
            "tasks": [
                {
                    "task_id": "",
                    "success": {
                        "next": "task2"
                    },
                    "failure": {
                        "next": "task3"
                    },
                    "task_inputs": [],
                    "process_inputs": []
                },
                {
                    "task_id": "task2",
                    "success": {
                        "next": "task2"
                    },
                    "failure": {
                        "next": "task3"
                    },
                    "task_inputs": [],
                    "process_inputs": []
                },
                {
                    "task_id": "task3",
                    "success": {
                        "next": "task2"
                    },
                    "failure": {
                        "next": "task3"
                    },
                    "task_inputs": [],
                    "process_inputs": []
                }
            ],
            "states": {
                "initial": "task2"
            }
        }
        "#;
        let errors = Plan::validate_plan(plan_json).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], "Task ID is empty for task: ...");
    }

    // #[test]
    // fn test_validate_plan_empty_task_id() {
    //     let plan = Plan {
    //         tasks: vec![Task {
    //             task_id: "".to_string(),
    //             // other fields...
    //         }],
    //         // other fields...
    //     };
    //     let errors = plan.validate_plan();
    //     assert_eq!(errors.len(), 1);
    //     assert_eq!(errors[0], "Task ID is empty for task: ...");
    // }
    //
    // #[test]
    // fn test_validate_plan_duplicate_task_id() {
    //     let plan = Plan {
    //         tasks: vec![
    //             Task {
    //                 task_id: "task1".to_string(),
    //                 // other fields...
    //             },
    //             Task {
    //                 task_id: "task1".to_string(),
    //                 // other fields...
    //             },
    //         ],
    //         // other fields...
    //     };
    //     let errors = plan.validate_plan();
    //     assert_eq!(errors.len(), 1);
    //     assert_eq!(errors[0], "Task ID 'task1' is not unique.");
    // }

    // #[test]
    // fn test_validate_plan_nonexistent_next_task() {
    //     let plan = Plan {
    //         tasks: vec![Task {
    //             task_id: "task1".to_string(),
    //             success: TaskOutcome {
    //                 next: "task2".to_string(),
    //                 // other fields...
    //             },
    //             failure: TaskOutcome {
    //                 next: "task3".to_string(),
    //                 // other fields...
    //             },
    //             // other fields...
    //         }],
    //         // other fields...
    //     };
    //     let errors = plan.validate_plan();
    //     assert_eq!(errors.len(), 2);
    //     assert_eq!(errors[0], "'success.next' 'task2' does not refer to an existing task.");
    //     assert_eq!(errors[1], "'failure.next' 'task3' does not refer to an existing task.");
    // }

    // #[test]
    // fn test_validate_plan_unreachable_task() {
    //     let plan = Plan {
    //         tasks: vec![
    //             Task {
    //                 task_id: "task1".to_string(),
    //                 success: TaskOutcome {
    //                     next: "task1".to_string(),
    //                     // other fields...
    //                 },
    //                 // other fields...
    //             },
    //             Task {
    //                 task_id: "task2".to_string(),
    //                 // other fields...
    //             },
    //         ],
    //         states: WorkflowState {
    //             initial: "task1".to_string(),
    //             // other fields...
    //         },
    //         // other fields...
    //     };
    //     let errors = plan.validate_plan();
    //     assert_eq!(errors.len(), 1);
    //     assert_eq!(errors[0], "Task 'task2' is not reachable from the initial task.");
    // }

    // #[test]
    // fn test_validate_plan_invalid_process_inputs() {
    //     let plan = Plan {
    //         tasks: vec![Task {
    //             task_id: "task1".to_string(),
    //             task_inputs: vec!["input1".to_string()],
    //             process_inputs: vec![1],
    //             // other fields...
    //         }],
    //         // other fields...
    //     };
    //     let errors = plan.validate_plan();
    //     assert_eq!(errors.len(), 1);
    //     assert_eq!(errors[0], "'process_inputs' index '1' is out of bounds for task_inputs in task 'task1'.");
    // }
}
