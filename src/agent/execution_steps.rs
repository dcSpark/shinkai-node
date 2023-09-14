use crate::tools::router::ShinkaiTool;
use serde_json::Value as JsonValue;

// 1. We start with all execution plans filling the context and saving the user's message with an InitialExecutionStep
// 2. We then iterate through the rest of the steps.
// 3. If they're Inference steps, we just record the task message from the original bootstrap plan, and get a name for the output which we will assign before adding the result into the context.
// 4. If they're Tool steps, we execute the tool and use the output_name before adding the result to the context
// 5. Once all execution steps have been processed, we inference the LLM one last time, providing it the whole context + the user's initial message, and tell it to respond to the user using the context.
// 6. We then save the final execution context (eventually adding summarization/pruning) as the Job's persistent context, save all of the prompts/responses from the LLM in the step history, and add a ShinkaiMessage into the Job inbox with the final response.

/// Initial data to be used for execution, including filling up the context
pub struct InitialExecutionStep {
    initial_context: Option<String>,
    user_message: String,
}

/// An execution step that the LLM decided it could perform without any tools.
pub struct InferenceExecutionStep {
    plan_task_message: String,
    output_name: String,
}

/// An execution step that requires executing a ShinkaiTool.
/// Of note `output_name` is used to label the output of the tool with an alternate name
/// before adding the results into the execution context
pub struct ToolExecutionStep {
    tool: ShinkaiTool,
    output_name: String,
}

pub enum ExecutionStep {
    Initial(InitialExecutionStep),
    Inference(InferenceExecutionStep),
    Tool(ToolExecutionStep),
}
