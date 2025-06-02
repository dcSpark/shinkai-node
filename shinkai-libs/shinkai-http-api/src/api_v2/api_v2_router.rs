use crate::node_commands::NodeCommand;

use super::api_v2_handlers_ext_agent_offers::ext_agent_offers_routes;
use super::api_v2_handlers_general::general_routes;
use super::api_v2_handlers_jobs::job_routes;
use super::api_v2_handlers_mcp_servers::mcp_server_routes;
use super::api_v2_handlers_ngrok::ngrok_routes;
use super::api_v2_handlers_oauth::oauth_routes;
use super::api_v2_handlers_prompts::prompt_routes;
#[cfg(feature = "swagger-ui")]
use super::api_v2_handlers_swagger_ui::swagger_ui_routes;
use super::api_v2_handlers_tools::tool_routes;
use super::api_v2_handlers_vecfs::vecfs_routes;
use super::api_v2_handlers_wallets::wallet_routes;
use super::{api_v2_handlers_cron::cron_routes, api_v2_handlers_mcp_servers::add_mcp_server_handler};
use async_channel::Sender;
use serde::Serialize;
use serde_json::{json, Value};

use warp::Filter;

pub fn v2_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let general_routes = general_routes(node_commands_sender.clone(), node_name.clone());
    let vecfs_routes = vecfs_routes(node_commands_sender.clone(), node_name.clone());
    let job_routes = job_routes(node_commands_sender.clone(), node_name.clone());
    let ext_agent_offers = ext_agent_offers_routes(node_commands_sender.clone());
    let wallet_routes = wallet_routes(node_commands_sender.clone());
    let custom_prompt = prompt_routes(node_commands_sender.clone());
    #[cfg(feature = "swagger-ui")]
    let swagger_ui_routes = swagger_ui_routes();
    let tool_routes = tool_routes(node_commands_sender.clone());
    let cron_routes = cron_routes(node_commands_sender.clone(), node_name.clone());
    let oauth_routes = oauth_routes(node_commands_sender.clone());
    let mcp_server_routes = mcp_server_routes(node_commands_sender.clone());
    let ngrok_routes = ngrok_routes(node_commands_sender.clone());

    #[cfg(feature = "swagger-ui")]
    return general_routes
        .or(vecfs_routes)
        .or(job_routes)
        .or(ext_agent_offers)
        .or(wallet_routes)
        .or(custom_prompt)
        .or(swagger_ui_routes)
        .or(tool_routes)
        .or(cron_routes)
        .or(oauth_routes)
        .or(mcp_server_routes)
        .or(ngrok_routes);

    #[cfg(not(feature = "swagger-ui"))]
    return general_routes
        .or(vecfs_routes)
        .or(job_routes)
        .or(ext_agent_offers)
        .or(wallet_routes)
        .or(custom_prompt)
        .or(tool_routes)
        .or(cron_routes)
        .or(oauth_routes)
        .or(mcp_server_routes)
        .or(ngrok_routes);
}

pub fn with_sender(
    sender: Sender<NodeCommand>,
) -> impl Filter<Extract = (Sender<NodeCommand>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || sender.clone())
}

pub fn with_node_name(node_name: String) -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || node_name.clone())
}

pub fn create_success_response<T: Serialize>(data: T) -> Value {
    json!(data)
}
