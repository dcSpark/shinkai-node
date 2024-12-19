use std::{collections::HashMap, sync::Arc};

use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{error::ToolError, tool_config::OAuth};

use super::execution_coordinator::handle_oauth;

pub async fn generate_execution_environment(
    db: Arc<SqliteManager>,
    llm_provider: String,
    app_id: String,
    tool_id: String,
    tool_router_key: String,
    instance_id: String,
    oauth: &Option<Vec<OAuth>>,
) -> Result<HashMap<String, String>, ToolError> {
    let mut envs = HashMap::new();

    let bearer = db.read_api_v2_key().unwrap_or_default().unwrap_or_default();
    envs.insert("BEARER".to_string(), bearer);
    envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
    envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());
    envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), instance_id.clone());
    envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider);

    let oauth = handle_oauth(oauth, &db, app_id.clone(), tool_id.clone(), tool_router_key.clone()).await?;

    envs.insert("SHINKAI_OAUTH".to_string(), oauth.to_string());

    Ok(envs)
}
