use std::sync::{Arc, Weak};
use std::time::Instant;
use std::env;

use crate::db::ShinkaiDB;
use crate::tools::error::ToolError;
use crate::tools::js_toolkit::JSToolkit;
use crate::tools::rust_tools::RustTool;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::tools::tool_router::ToolRouter;
use crate::tools::workflow_tool::WorkflowTool;
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::MapVectorResource;

use super::workflows_data;

pub async fn start(
    tool_router: &mut ToolRouter,
    generator: Box<dyn EmbeddingGenerator>,
    db: Weak<ShinkaiDB>,
    profile: ShinkaiName,
) -> Result<(), ToolError> {
    if tool_router.started {
        return Err(ToolError::AlreadyStarted);
    }

    let name = "Tool Router";
    let desc = Some("Enables performing vector searches to find relevant tools.");
    let source = VRSourceReference::None;

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;

    let mut routing_resource = MapVectorResource::new_empty(name, desc, source, true);

    add_rust_tools(&mut routing_resource, generator.box_clone()).await;

    if let Some(db) = db.upgrade() {
        add_static_workflows(&mut routing_resource, generator.box_clone(), db.clone(), profile.clone()).await;
        add_js_tools(&mut routing_resource, generator, db, profile.clone()).await;
    }

    tool_router.routing_resources.insert(profile.to_string(), routing_resource);
    tool_router.started = true;
    Ok(())
}

async fn add_rust_tools(routing_resource: &mut MapVectorResource, generator: Box<dyn EmbeddingGenerator>) {
    let rust_tools = RustTool::static_tools(generator).await;

    for tool in rust_tools {
        let shinkai_tool = ShinkaiTool::Rust(tool.clone(), true);
        let _ = routing_resource.insert_text_node(
            shinkai_tool.tool_router_key(),
            shinkai_tool.to_json().unwrap(),
            None,
            tool.tool_embedding.clone(),
            &vec![],
        );
    }
}

async fn add_static_workflows(
    routing_resource: &mut MapVectorResource,
    generator: Box<dyn EmbeddingGenerator>,
    db: Arc<ShinkaiDB>,
    profile: ShinkaiName,
) {
    let model_type = generator.model_type();
    let start_time = Instant::now();

    if let EmbeddingModelType::OllamaTextEmbeddingsInference(
        OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
    ) = model_type
    {
        let data = workflows_data::WORKFLOWS_JSON;
        let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
        let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        for item in json_array {
            let shinkai_tool_value = &item["shinkai_tool"];
            let shinkai_tool: ShinkaiTool =
                serde_json::from_value(shinkai_tool_value.clone()).expect("Failed to parse shinkai_tool");

            let embedding_value = &item["embedding"];
            let embedding: Embedding =
                serde_json::from_value(embedding_value.clone()).expect("Failed to parse embedding");

            let _ = routing_resource.insert_text_node(
                shinkai_tool.tool_router_key(),
                shinkai_tool.to_json().unwrap(),
                None,
                embedding,
                &vec![],
            );

            if let ShinkaiTool::Workflow(workflow_tool, _) = &shinkai_tool {
                if let Err(e) = db.save_workflow(workflow_tool.workflow.clone(), profile.clone()) {
                    eprintln!("Error saving workflow to DB: {:?}", e);
                }
            }
        }
    } else {
        let workflows = WorkflowTool::static_tools();
        println!("Number of static workflows: {}", workflows.len());

        for workflow_tool in workflows {
            let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), true);

            let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
                embedding
            } else {
                generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap()
            };

            let _ = routing_resource.insert_text_node(
                shinkai_tool.tool_router_key(),
                shinkai_tool.to_json().unwrap(),
                None,
                embedding,
                &vec![],
            );

            if let Err(e) = db.save_workflow(workflow_tool.workflow.clone(), profile.clone()) {
                eprintln!("Error saving workflow to DB: {:?}", e);
            }
        }
    }

    let duration = start_time.elapsed();
    if env::var("LOG_ALL").unwrap_or_default() == "1" {
        println!("Time taken to generate static workflows: {:?}", duration);
    }
}

async fn add_js_tools(
    routing_resource: &mut MapVectorResource,
    generator: Box<dyn EmbeddingGenerator>,
    db: Arc<ShinkaiDB>,
    profile: ShinkaiName,
) {
    let tools = built_in_tools::get_tools();
    for (name, definition) in tools {
        let toolkit = JSToolkit::new(&name, vec![definition]);
        db.add_jstoolkit(toolkit.clone(), profile.clone()).unwrap();
    }

    match db.all_tools_for_user(&profile) {
        Ok(tools) => {
            for tool in tools {
                if let ShinkaiTool::JS(mut js_tool, isEnabled) = tool {
                    let js_lite_tool = js_tool.to_without_code();
                    let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool, isEnabled);

                    let embedding = if let Some(embedding) = js_tool.embedding.clone() {
                        embedding
                    } else {
                        let new_embedding = generator
                            .generate_embedding_default(&shinkai_tool.format_embedding_string())
                            .await
                            .unwrap();
                        js_tool.embedding = Some(new_embedding.clone());
                        if let Err(e) = db.add_shinkai_tool(ShinkaiTool::JS(js_tool.clone(), isEnabled), profile.clone()) {
                            eprintln!("Error updating JS tool in DB: {:?}", e);
                        }
                        new_embedding
                    };

                    let _ = routing_resource.insert_text_node(
                        shinkai_tool.tool_router_key(),
                        shinkai_tool.to_json().unwrap(),
                        None,
                        embedding,
                        &vec![],
                    );
                }
            }
        }
        Err(e) => eprintln!("Error fetching JS tools: {:?}", e),
    }
}