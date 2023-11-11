#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use shinkai_node::planner::runner::{Plan, Task};
    use std::{pin::Pin, sync::Arc};
    use tokio::sync::Mutex;

    fn mock_execute_task(_task: &Task) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>> {
        // Mock implementation here
        eprintln!("Mock task execution OK");
        Box::pin(async { Ok(()) })
    }

    #[tokio::test]
    async fn test_process_plan() {
        let json = r#"
        {
            "tasks": [
                {
                    "task_id": "task_id_1",
                    "task_inputs_schema": ["string", "string"],
                    "task_inputs": ["input1", "input2"],
                    "description": "some description",
                    "task_query": "Find all the news related to {}",
                    "query_inputs": [0],
                    "task_query_output": ["some_result with: one %% two %% three"],
                    "post_processing:": [
                        {
                            "mini_tool": "splitter",
                            "param": "%%"
                        },
                        {
                            "mini_tool": "regex",
                            "param": "some_regex"
                        }
                    ],
                    "post_processing_results": [
                        "one", "two", "three"
                    ],
                    "task_outputs": [
                        {
                            "task_inputs": "string",
                            "index": 1
                        },
                        {
                            "post_processing_results": "string",
                            "index": 0
                        },
                        {
                            "post_processing_results": "string",
                            "index": 1
                        },
                        {
                            "post_processing_results": "string",
                            "index": 2
                        }
                    ],
                    "state": "completed",
                    "success": {
                        "next": "task_id_2",
                        "message": {
                            "query": "Step completed successfully with result {}",
                            "refs": ["output:0"]
                        }
                    },
                    "failure": {
                        "next": "",
                        "message": {
                            "query": "This task failed for input {}. Executing error handling.",
                            "refs": ["input:0"]
                        }
                    }
                },
                {
                    "task_id": "task_id_2",
                    "task_inputs": ["string", "string"],
                    "description": "some description",
                    "task_query": "Find all the news related to {}",
                    "query_inputs": [0],
                    "task_query_output": ["some_result with: one %% two %% three"],
                    "post_processing:": [
                        {
                            "mini_tool": "splitter",
                            "param": "%%"
                        },
                        {
                            "mini_tool": "regex",
                            "param": "some_regex"
                        }
                    ],
                    "post_processing_results": [
                        "one", "two", "three"
                    ],
                    "task_outputs": [
                        {
                            "task_inputs": "string",
                            "index": 1
                        },
                        {
                            "post_processing_results": "string",
                            "index": 0
                        },
                        {
                            "post_processing_results": "string",
                            "index": 1
                        },
                        {
                            "post_processing_results": "string",
                            "index": 2
                        }
                    ],
                    "state": "completed",
                    "success": {
                        "next": "task_id_2",
                        "message": {
                            "query": "Step completed successfully with result {}",
                            "refs": ["output:0"]
                        }
                    },
                    "failure": {
                        "next": "",
                        "message": {
                            "query": "This task failed for input {}. Executing error handling.",
                            "refs": ["input:0"]
                        }
                    }
                },
            ],
            "states": {
                "initial": {
                    "task_id": "task_id_1",
                    "inputs": ["input1", "input2"]
                },
                "processed": [],
                "completed": false
            },
            "iteration_control": {
                "max_iterations": 10,
                "current_iteration": 0
            }
        }
        "#;

        // first validate plan
        let errors = Plan::validate_plan(json).unwrap();
        eprintln!("Errors: {:?}", errors);
        assert_eq!(errors.len(), 0);

        let mut plan: Plan = serde_json::from_str(json).unwrap();
        plan.execute_task = mock_execute_task;

        let plan = Arc::new(Mutex::new(plan));
        let _ = Plan::process_plan(Arc::clone(&plan)).await;

        let plan_guard = plan.lock().await;
        assert_eq!(plan_guard.states.processed.len(), 2);
        assert!(plan_guard.states.completed);
    }
}
