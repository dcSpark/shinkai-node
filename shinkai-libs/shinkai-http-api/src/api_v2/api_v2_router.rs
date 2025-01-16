use crate::node_commands::NodeCommand;

use super::api_v2_handlers_app_files::app_files_routes;
use super::api_v2_handlers_cron::cron_routes;
use super::api_v2_handlers_ext_agent_offers::ext_agent_offers_routes;
use super::api_v2_handlers_general::general_routes;
use super::api_v2_handlers_jobs::job_routes;
use super::api_v2_handlers_oauth::oauth_routes;
use super::api_v2_handlers_prompts::prompt_routes;
use super::api_v2_handlers_sheets::sheets_routes;
use super::api_v2_handlers_swagger_ui::swagger_ui_routes;
use super::api_v2_handlers_tools::{
    add_shinkai_tool_handler, disable_all_tools_handler, enable_all_tools_handler, get_shinkai_tool_handler,
    list_all_shinkai_tools_handler, list_playground_tools_handler, set_shinkai_tool_handler, tool_routes,
};
use super::api_v2_handlers_vecfs::vecfs_routes;
use super::api_v2_handlers_wallets::wallet_routes;
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
    let swagger_ui_routes = swagger_ui_routes();
    let sheets_routes = sheets_routes(node_commands_sender.clone());
    let tool_routes = tool_routes(node_commands_sender.clone());
    let cron_routes = cron_routes(node_commands_sender.clone(), node_name.clone());
    let oauth_routes = oauth_routes(node_commands_sender.clone());
    let app_files_routes = app_files_routes(node_commands_sender.clone());
    let api_enable_all_tools = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v2" / "enable_all_tools")
            .and(warp::post())
            .and(with_sender(node_commands_sender))
            .and(warp::header::<String>("authorization"))
            .and_then(enable_all_tools_handler)
    };
    let api_disable_all_tools = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v2" / "disable_all_tools")
            .and(warp::post())
            .and(with_sender(node_commands_sender))
            .and(warp::header::<String>("authorization"))
            .and_then(disable_all_tools_handler)
    };
    general_routes
        .or(vecfs_routes)
        .or(job_routes)
        .or(ext_agent_offers)
        .or(wallet_routes)
        .or(custom_prompt)
        .or(swagger_ui_routes)
        .or(sheets_routes)
        .or(tool_routes)
        .or(cron_routes)
        .or(oauth_routes)
        .or(app_files_routes)
        .or(api_enable_all_tools)
        .or(api_disable_all_tools)
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
