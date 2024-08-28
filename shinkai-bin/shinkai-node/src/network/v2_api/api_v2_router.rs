use crate::network::node_commands::NodeCommand;

use super::api_v2_handlers_jobs::job_routes;
use super::api_v2_handlers_vecfs::vecfs_routes;
use super::api_v2_handlers_workflows::workflows_routes;
use super::{api_v2_handlers_general::general_routes, api_v2_handlers_subscriptions::subscriptions_routes};
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
    let subscriptions_routes = subscriptions_routes(node_commands_sender.clone());
    let workflows_routes = workflows_routes(node_commands_sender.clone());

    general_routes
        .or(vecfs_routes)
        .or(job_routes)
        .or(subscriptions_routes)
        .or(workflows_routes)
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
