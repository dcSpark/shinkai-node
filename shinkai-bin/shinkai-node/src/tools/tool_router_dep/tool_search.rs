use std::sync::Arc;

use crate::db::ShinkaiDB;
use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::tools::tool_router::ToolRouter;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::{VectorResourceCore, VectorResourceSearch};

pub fn syntactic_vector_search(
    tool_router: &ToolRouter,
    profile: &ShinkaiName,
    query: Embedding,
    num_of_results: u64,
    data_tag_names: &Vec<String>,
) -> Result<Vec<ShinkaiTool>, ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    let routing_resource = tool_router
        .routing_resources
        .get(&profile.to_string())
        .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
    let nodes = routing_resource.syntactic_vector_search(query, num_of_results, data_tag_names);
    Ok(tool_router.ret_nodes_to_tools(&nodes))
}

pub fn vector_search(
    tool_router: &ToolRouter,
    profile: &ShinkaiName,
    query: Embedding,
    num_of_results: u64,
) -> Result<Vec<ShinkaiTool>, ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    let routing_resource = tool_router
        .routing_resources
        .get(&profile.to_string())
        .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
    let nodes = routing_resource.vector_search(query, num_of_results);

    for node in &nodes {
        if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.node.get_text_content()?) {
            eprintln!(
                "Node Score: {}, Toolkit Name: {}",
                node.score,
                shinkai_tool.toolkit_name()
            );
        }
    }
    Ok(tool_router.ret_nodes_to_tools(&nodes))
}

pub async fn workflow_search(
    tool_router: &mut ToolRouter,
    profile: ShinkaiName,
    embedding_generator: Box<dyn EmbeddingGenerator>,
    db: Arc<ShinkaiDB>,
    query: Embedding,
    name_query: &str,
    num_of_results: u64,
) -> Result<Vec<ShinkaiTool>, ToolError> {
    if !tool_router.started {
        let _ = tool_router
            .start(embedding_generator, Arc::downgrade(&db), profile.clone())
            .await;
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    let routing_resource = tool_router
        .routing_resources
        .get(&profile.to_string())
        .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;

    let vector_nodes = routing_resource.vector_search(query, num_of_results);

    let mut name_similarity_results = vec![];
    for node in routing_resource.get_root_nodes() {
        if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.get_text_content()?) {
            if let ShinkaiTool::Workflow(_, _) = shinkai_tool {
                let name = shinkai_tool.name().to_lowercase();
                let query = name_query.to_lowercase();
                if name.contains(&query) {
                    let similarity_score = (query.len() as f64 / name.len() as f64) as f32;
                    name_similarity_results.push((shinkai_tool, similarity_score));
                }
            }
        }
    }

    let mut combined_results = vec![];
    let mut seen_keys = std::collections::HashSet::new();

    for node in vector_nodes {
        if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.node.get_text_content()?) {
            if let ShinkaiTool::Workflow(_, _) = shinkai_tool {
                let key = shinkai_tool.tool_router_key();
                if seen_keys.insert(key.clone()) {
                    combined_results.push((shinkai_tool, node.score));
                }
            }
        }
    }

    for (shinkai_tool, similarity_score) in name_similarity_results {
        let key = shinkai_tool.tool_router_key();
        if seen_keys.insert(key.clone()) {
            combined_results.push((shinkai_tool, similarity_score));
        }
    }

    combined_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(combined_results
        .into_iter()
        .map(|(tool, _)| tool)
        .take(num_of_results as usize)
        .collect())
}
