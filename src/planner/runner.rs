use futures::Future;
// Example of a plan implementation
// {
//     "tasks": [
//       {
//         "task_id": "task_id_1",
//         "description": "some description",
//         "task_query": "Find all the news related to {}",
//         "task_inputs": ["string", "string"],
//         "process_inputs": [0],
//         "task_outputs": ["string", "string"],
//         "success": {
//           "next": "task_id_2",
//           "message": {
//             "query": "Step completed successfully with result {}",
//             "ref": ["output:0"]
//           }
//         },
//         "failure": {
//           "next": "error_task_id",
//           "message": {
//             "query": "This task failed for input {}. Executing error handling.",
//             "ref": ["input:0"]
//           }
//         }
//       },
//       {
//         "task_id": "task_id_2",
//         "description": "Process inputA_response and inputB",
//         "task_inputs": ["string", "string"],
//         "process_inputs": [0, 1]
//       }
//       // ... Additional tasks ...
//     ],
//     "states": {
//       "initial": "task_id_1",
//       "processed": [],
//       "completed": false
//     },
//     "iterationControl": {
//       "maxIterations": 10,
//       "currentIteration": 0
//     }
//   }

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

// Define the structure of your JSON
#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub task_id: String,
    pub description: String,
    pub task_query: String,
    pub task_inputs: Vec<String>,   // Input types
    pub process_inputs: Vec<usize>, // Indices of inputs to process
    pub task_outputs: Vec<String>,  // Output types
    pub success: TaskOutcome,
    pub failure: TaskOutcome,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskOutcome {
    pub next: String,
    pub message: OutcomeMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OutcomeMessage {
    pub query: String,
    pub refs: Vec<String>,
}

pub type ExecuteTaskFn = fn(&Task) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>;

fn default_execute_task_fn() -> ExecuteTaskFn {
    // Provide a default function
    fn task_fn(task: &Task) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>> {
        let task_id = task.task_id.clone();
        Box::pin(async move {
            println!("Default task execution for {}", task_id);
            Ok(())
        })
    }
    task_fn
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Plan {
    pub tasks: Vec<Task>,
    pub states: WorkflowState,
    pub iteration_control: IterationControl,
    #[serde(skip_serializing, skip_deserializing, default = "default_execute_task_fn")]
    pub execute_task: ExecuteTaskFn,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkflowState {
    pub initial: String,
    pub processed: Vec<String>,
    pub completed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IterationControl {
    pub max_iterations: usize,
    pub current_iteration: usize,
}

impl Plan {
    pub fn process_plan(plan: Arc<Mutex<Plan>>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut current_task_id;
            let mut plan_guard = plan.lock().await;
            current_task_id = plan_guard.states.initial.clone();

            while !plan_guard.states.completed
                && plan_guard.iteration_control.current_iteration < plan_guard.iteration_control.max_iterations
            {
                if let Some(task) = plan_guard.tasks.iter().find(|t| &t.task_id == &current_task_id) {
                    let task = task.clone(); // Clone the task to use it later
                    let task_id = task.task_id.clone(); // Clone the task_id to use it later
                    let next_task_id = task.success.next.clone(); // Clone the next_task_id to use it later

                    match (plan_guard.execute_task)(&task).await {
                        Ok(_) => {
                            plan_guard.states.processed.push(task_id);
                            current_task_id = next_task_id;
                        }
                        Err(_) => {
                            current_task_id = task.failure.next.clone();
                        }
                    }
                } else {
                    println!("Task not found: {}", current_task_id);
                    plan_guard.states.completed = true;
                    break;
                }

                plan_guard.iteration_control.current_iteration += 1;

                // Check if there are more tasks to process
                if current_task_id.is_empty()
                    || plan_guard
                        .tasks
                        .iter()
                        .find(|t| &t.task_id == &current_task_id)
                        .is_none()
                {
                    plan_guard.states.completed = true;
                }
            }
        })
    }
}
