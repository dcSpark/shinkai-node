use std::env;

use regex::Regex;
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

    pub fn get_db_key(&self) -> String {
        format!("{}:::{}", self.workflow.name, self.workflow.version)
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
    pub fn static_tools() -> Vec<(Self, bool)> {
        let is_testing = env::var("IS_TESTING")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        if is_testing {
            vec![
                (Self::get_extensive_summary_workflow(), true),
                (Self::get_hyde_inference_workflow(), true),
                (Self::baml_answer_with_citations(), false),
                (Self::answer_with_citations_workflow(), true),
            ]
        } else {
            vec![
                (Self::get_extensive_summary_workflow(), true),
                (Self::get_hyde_inference_workflow(), true),
                (Self::baml_answer_with_citations(), false),
                (Self::answer_with_citations_workflow(), true),
            ]
        }
    }

    fn get_extensive_summary_workflow() -> Self {
        let raw_workflow = r#"
            workflow Extensive_summary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            } @@official.shinkai
        "#;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Reviews in depth all the content to generate a summary.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_hyde_inference_workflow() -> Self {
        let raw_workflow = r#"
            workflow Hyde_inference v0.1 {
                step Initialize {
                    $PROMPT = "write a passage to answer the question: "
                    $HYDE_PROMPT = call concat($PROMPT, $INPUT)
                    $HYDE_PASSAGE = call inference_no_ws($HYDE_PROMPT)
                    $HYDE_INPUT = call concat($INPUT, ". ", $HYDE_PASSAGE )
                    $EMBEDDINGS = call search_embeddings_in_job_scope($HYDE_INPUT)
                }
                step Summarize {
                    $CONNECTOR = "\nLeverage the following information to answer the previous query: --- start ---"
                    $NEW_INPUT = call concat($INPUT, $CONNECTOR, $EMBEDDINGS) 
                    $RESULT = call inference($NEW_INPUT)
                }
            } @@official.shinkai
        "#;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description =
            Some("Generates a passage to answer a question and uses embeddings to refine the answer.".to_string());

        WorkflowTool::new(workflow)
    }

    fn baml_answer_with_citations() -> Self {
        let dsl_content = r###"
        class Citation {
            citation_id int
            document_reference string @description(#"The name of the document and the page number that supports the answer e.g., FILENAME: Page [PAGE NUMBER]. This is a reference. You must mention it NO MATTER WHAT."#)
            relevantTextFromDocument string @alias("relevantSentenceFromDocument") @description(#"The relevant text from the document that supports the answer. This is part of the citation. You must quote it EXACTLY as it appears in the document with any special characters it contains. You may cite a part of the sentence. The text should be contiguous and not broken up. You may NOT summarize or skip sentences. If you need to skip a sentence, start a new citation instead."#)
        }

        class Answer {
            answersInText Citation[] @alias("relevantSentencesFromText")
            answer AnswerWithCitations @description(#"An answer to the user's question that MUST cite sources from the relevantSentencesFromText. Like [0]. If multiple citations are needed, write them like [0][1][2]."#)
        }

        class AnswerWithCitations {
            brief_introduction Paragraph @description(#"3-4 long sentences. Must use the citations in the text."#)
            extensive_body Paragraph[] @description(#"At least 3-6 long sentences. The more the better. Must use the citations in the text."#)
            conclusion Paragraph[] @description(#"1-3 long sentences. Must use the citations in the text."#)
        }

        class Paragraph {
            sentences string[]
        }

        class Document {
            file string
            text string
            reference string
        }
        class Context {
            documents Document[]
            question string
        }

        function AnswerQuestion(context: Context) -> Answer {
            client ShinkaiProvider

            prompt #"
                Out of the given content, do your best to answer the question.

                CONTEXT:
                {% for document in context.documents %}
                ----
                DOCUMENT NAME: {{  document.file }}
                PARTIAL TEXT: {{ document.text }}
                DOCUMENT REFERENCE: {{ document.reference }}
                ---
                {% endfor %}
                
                {{ ctx.output_format }}

                QUESTION: {{ context.question }}. Citing the references no matter what e.g., [0]. If multiple citations are needed, write them like [0][1][2].

                ANSWER:
                {{ _.role("user") }}
            "#
        }
        "###;

        // Input needs to be a serialized json with the following structure:
        //
        //  "documents": Document[]
        //  "question": "The question to answer"

        let re = Regex::new(r#"""#).unwrap();
        let escaped_dsl_content = re.replace_all(dsl_content.trim(), r#"\""#);

        let raw_workflow = format!(r##"
            workflow baml_answer_with_citations v0.1 {{
                step Initialize {{
                    $DSL = "{}"
                    $PARAM = "context"
                    $FUNCTION = "AnswerQuestion"
                    $RESULT = call baml_inference($INPUT, "", "", $DSL, $FUNCTION, $PARAM)
                }}
            }} @@official.shinkai
        "##, escaped_dsl_content);

        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates an answer to a question with citations from the provided content using BAML. The answer includes quotes from the content as citations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn answer_with_citations_workflow() -> Self {
        let raw_workflow = r##"
            workflow RAG v0.1 {
                step Initialize {
                    $FILE_PIECES = call process_embeddings_in_job_scope_with_metadata()
                    
                    $LLM_INPUT = call generate_json_map("question", $INPUT, "documents", $FILE_PIECES)
                    
                    $LLM_RESPONSE = call baml_answer_with_citations($LLM_INPUT)
                    
                    $JINJA = "# Introduction\n{%- for sentence in answer.brief_introduction.sentences %}\n{{ sentence }}\n{%- endfor %}\n\n# Body\n{%- for section in answer.extensive_body %}\n## Section {{ loop.index }}\n{%- for sentence in section.sentences %}\n{{ sentence }}\n{%- endfor %}\n{%- endfor %}\n\n# Conclusion\n{%- for section in answer.conclusion %}\n{{ section.sentences[0] }}\n{%- endfor %}\n\n# Citations\n{%- for citation in relevantSentencesFromText %}\n[{{ citation.citation_id }}]: {{ citation.relevantSentenceFromDocument }}\n{%- endfor %}"
                    
                    $RESULT = call shinkai__json-to-md("message",$LLM_RESPONSE,"template",$JINJA)
                }
            } @@official.shinkai
        "##;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates an answer to a question with citations from the provided content using RAG workflow.".to_string());

        WorkflowTool::new(workflow)
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

    #[test]
    fn test_get_db_key() {
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

        assert_eq!(workflow_tool.get_db_key(), "ExtensiveSummary:::v0.1");
    }
}
