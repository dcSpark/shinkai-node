use shinkai_dsl::{dsl_schemas::Workflow, parser::parse_workflow};
use shinkai_vector_resources::embeddings::Embedding;

use super::argument::ToolArgument;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowTool {
    pub workflow: Workflow,
    pub embedding: Option<Embedding>,
}

impl WorkflowTool {
    pub fn new(workflow: Workflow) -> Self {
        WorkflowTool {
            workflow,
            embedding: None,
        }
    }

    pub fn get_name(&self) -> String {
        self.workflow.name.clone()
    }

    pub fn get_description(&self) -> String {
        self.workflow.description.clone().unwrap_or_default()
    }

    pub fn get_input_args(&self) -> Vec<ToolArgument> {
        if self.workflow.raw.contains("$INPUT") {
            vec![ToolArgument::new(
                "input".to_string(),
                "string".to_string(),
                "Input for the workflow".to_string(),
                true,
            )]
        } else {
            Vec::new()
        }
    }

    // Additional methods that might be useful
    pub fn get_embedding(&self) -> Option<Embedding> {
        self.embedding.clone()
    }

    pub fn format_embedding_string(&self) -> String {
        let mut embedding_string = format!("{} {}\n", self.get_name(), self.get_description());
        embedding_string.push_str("Input Args:\n");

        for arg in self.get_input_args() {
            embedding_string.push_str(&format!("- {} : {}\n", arg.name, arg.description));
        }

        embedding_string
    }
}

impl WorkflowTool {
    pub fn static_tools() -> Vec<Self> {
        let mut tools = Vec::new();

        let raw_workflow = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Reviews in depth all the content to generate a summary.".to_string());

        tools.push(WorkflowTool::new(workflow));

        // Add more workflows as needed
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_workflow_tool() {
        let raw_workflow = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;

        let workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        let workflow_tool = WorkflowTool::new(workflow);

        let serialized = serde_json::to_string(&workflow_tool).expect("Failed to serialize WorkflowTool");
        println!("{}", serialized);

        // Optionally, you can add assertions to check the serialized output
        assert!(serialized.contains("ExtensiveSummary"));
    }
}