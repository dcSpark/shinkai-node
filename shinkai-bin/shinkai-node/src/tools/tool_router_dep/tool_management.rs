use std::sync::Arc;

use crate::db::ShinkaiDB;
use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::tools::tool_router::ToolRouter;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::NodeContent;
use shinkai_vector_resources::vector_resource::VectorResourceCore;
use shinkai_vector_resources::vector_resource::VectorResourceSearch;

pub fn add_shinkai_tool(
    tool_router: &mut ToolRouter,
    profile: &ShinkaiName,
    shinkai_tool: &ShinkaiTool,
    embedding: Embedding,
) -> Result<(), ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    let routing_resource = tool_router
        .routing_resources
        .get_mut(&profile.to_string())
        .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
    let data = shinkai_tool.to_json()?;
    let router_key = shinkai_tool.tool_router_key();
    let metadata = None;

    match routing_resource.get_root_node(router_key.clone()) {
        Ok(_) => {
            return Err(ToolError::ToolAlreadyInstalled(data.to_string()));
        }
        Err(_) => {
            routing_resource._insert_kv_without_tag_validation(
                &router_key,
                NodeContent::Text(data),
                metadata,
                &embedding,
                &vec![],
            );
        }
    }

    Ok(())
}

pub fn delete_shinkai_tool(
    tool_router: &mut ToolRouter,
    profile: &ShinkaiName,
    tool_name: &str,
    toolkit_name: &str,
) -> Result<(), ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    let routing_resource = tool_router
        .routing_resources
        .get_mut(&profile.to_string())
        .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
    let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
    routing_resource.print_all_nodes_exhaustive(None, false, false);
    routing_resource.remove_node_dt_specified(key, None, true)?;
    Ok(())
}

pub async fn add_js_toolkit(
    tool_router: &mut ToolRouter,
    profile: &ShinkaiName,
    toolkit: Vec<ShinkaiTool>,
    generator: Box<dyn EmbeddingGenerator>,
) -> Result<(), ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    for tool in toolkit {
        if let ShinkaiTool::JS(mut js_tool, _) = tool.clone() {
            let js_lite_tool = js_tool.to_without_code();
            let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool, true);

            let embedding = if let Some(embedding) = js_tool.embedding.clone() {
                embedding
            } else {
                let new_embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                js_tool.embedding = Some(new_embedding.clone());
                new_embedding
            };

            tool_router.add_shinkai_tool(&profile, &tool, embedding)?;
        }
    }
    Ok(())
}

pub fn remove_js_toolkit(
    tool_router: &mut ToolRouter,
    profile: &ShinkaiName,
    toolkit: Vec<ShinkaiTool>,
) -> Result<(), ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
    for tool in toolkit {
        if let ShinkaiTool::JS(js_tool, is_enabled) = tool {
            let js_lite_tool = js_tool.to_without_code();
            let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool, is_enabled);
            tool_router.delete_shinkai_tool(&profile, &shinkai_tool.name(), &shinkai_tool.toolkit_name())?;
        }
    }
    Ok(())
}

pub fn get_shinkai_tool(
    tool_router: &ToolRouter,
    profile: &ShinkaiName,
    tool_name: &str,
    toolkit_name: &str,
) -> Result<ShinkaiTool, ToolError> {
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
    let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
    let node = routing_resource.get_root_node(key)?;
    ShinkaiTool::from_json(node.get_text_content()?)
}

pub fn get_default_tools(tool_router: &ToolRouter, profile: &ShinkaiName) -> Result<Vec<ShinkaiTool>, ToolError> {
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

    let mut default_tools = Vec::new();

    let math_tool_key = ShinkaiTool::gen_router_key(
        "shinkai__math_expression_evaluator".to_string(),
        "shinkai-tool-math-exp".to_string(),
    );
    if let Ok(node) = routing_resource.get_root_node(math_tool_key) {
        if let Ok(tool) = ShinkaiTool::from_json(node.get_text_content()?) {
            default_tools.push(tool);
        }
    }

    // Add more default tools here if needed in the future

    Ok(default_tools)
}

/// Returns all available JS tools for a given user profile
pub fn all_available_js_tools(
    tool_router: &ToolRouter,
    profile: &ShinkaiName,
    db: Arc<ShinkaiDB>,
) -> Result<Vec<ShinkaiTool>, ToolError> {
    if !tool_router.started {
        return Err(ToolError::NotStarted);
    }

    let profile = profile
        .extract_profile()
        .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;

    match db.all_tools_for_user(&profile) {
        Ok(tools) => {
            let js_tools: Vec<ShinkaiTool> = tools
                .into_iter()
                .filter_map(|tool| match tool {
                    ShinkaiTool::JS(_, _) | ShinkaiTool::JSLite(_, _) => Some(tool),
                    _ => None,
                })
                .collect();
            Ok(js_tools)
        }
        Err(e) => Err(ToolError::DatabaseError(e.to_string())),
    }
}
