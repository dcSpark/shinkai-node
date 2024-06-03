use super::{agent::Agent, error::AgentError};
use crate::tools::router::ShinkaiTool;
use std::collections::HashMap;

// 1. We start with all execution plans filling the context and saving the user's message with an InitialExecutionStep
// 2. We then iterate through the rest of the steps.
// 3. If they're Inference steps, we just record the task message from the original bootstrap plan, and get a name for the output which we will assign before adding the result into the context.
// 4. If they're Tool steps, we execute the tool and use the output_name before adding the result to the context
// 5. Once all execution steps have been processed, we inference the LLM one last time, providing it the whole context + the user's initial message, and tell it to respond to the user using the context.
// 6. We then save the final execution context (eventually adding summarization/pruning) as the Job's persistent context, save all of the prompts/responses from the LLM in the step history, and add a ShinkaiMessage into the Job inbox with the final response.

/// Struct that executes a plan (Vec<ExecutionStep>) generated from the analysis phase
#[derive(Clone, Debug)]
pub struct PlanExecutor<'a> {
    agent: &'a Agent,
    execution_context: HashMap<String, String>,
    user_message: String,
    execution_plan: Vec<ExecutionStep>,
    inference_trace: Vec<String>,
}

impl<'a> PlanExecutor<'a> {
    pub fn new(agent: &'a Agent, execution_plan: &Vec<ExecutionStep>) -> Result<Self, AgentError> {
        match execution_plan.get(0) {
            Some(ExecutionStep::Initial(initial_step)) => {
                let mut execution_plan = execution_plan.to_vec();
                let execution_context = initial_step.initial_execution_context.clone();
                let user_message = initial_step.user_message.clone();
                execution_plan.remove(0); // Remove the initial step
                Ok(Self {
                    agent,
                    execution_context,
                    user_message,
                    execution_plan,
                    inference_trace: vec![],
                })
            }
            _ => Err(AgentError::MissingInitialStepInExecutionPlan),
        }
    }

    // TODO: Properly implement this once we have jobs update for context + agent infernece/use tool
    /// Executes the plan step-by-step, performing all inferencing & tool calls.
    /// All content sent for inferencing and all responses from the LLM are saved in self.inference_trace
    pub async fn execute_plan(&mut self) -> Result<(), AgentError> {
        for step in &self.execution_plan {
            match step {
                ExecutionStep::Inference(_inference_step) => {

                    // 1. Generate the content to be sent using prompt generator/self/step
                    // PromptGenerator::...

                    // 2. Save the content to be sent to the LLM
                    // self.inference_trace.push(content)

                    // 3. Inference
                    // self.agent
                    //     .inference(
                    //         inference_step.content.clone(),
                    //     )
                    //     .await;

                    // 4. Save full response to trace
                    // self.inference_trace.push(response)

                    // 5. Find & parse the JSON in the response
                }
                ExecutionStep::Tool(_tool_step) => {
                    // self.agent.use_tool(tool_step.tool.clone()).await?;
                }
                _ => (),
            }
        }
        Ok(())
    }
}

/// Initial data to be consumed while creating the PlanExecutor, primarily to fill up the initial_execution_context
#[derive(Clone, Debug)]
pub struct InitialExecutionStep {
    initial_execution_context: HashMap<String, String>,
    user_message: String,
}

/// An execution step that the LLM decided it could perform without any tools.
#[derive(Clone, Debug)]
pub struct InferenceExecutionStep {
    plan_task_message: String,
    output_name: String,
}

/// An execution step that requires executing a ShinkaiTool.
/// Of note `output_name` is used to label the output of the tool with an alternate name
/// before adding the results into the execution context
#[derive(Clone, Debug)]
pub struct ToolExecutionStep {
    tool: ShinkaiTool,
    output_name: String,
}

#[derive(Clone, Debug)]
pub enum ExecutionStep {
    Initial(InitialExecutionStep),
    Inference(InferenceExecutionStep),
    Tool(ToolExecutionStep),
}
