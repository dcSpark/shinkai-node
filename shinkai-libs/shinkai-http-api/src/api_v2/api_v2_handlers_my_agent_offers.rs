use async_channel::Sender;
use serde::Deserialize;
use serde_json::json;
use utoipa::{OpenApi, ToSchema};
use warp::http::StatusCode;
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::{create_success_response, with_sender};

pub fn my_agent_offers_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let generate_agent_from_prompt_route = warp::path("generate_agent_from_prompt")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(generate_agent_from_prompt_handler);

    generate_agent_from_prompt_route
}

#[derive(Deserialize, ToSchema)]
pub struct GenerateAgentFromPromptRequest {
    pub prompt: String,
}

#[utoipa::path(
    post,
    path = "/v2/generate_agent_from_prompt",
    request_body = GenerateAgentFromPromptRequest,
    responses(
        (status = 200, description = "Successfully set tool offering", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn generate_agent_from_prompt_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GenerateAgentFromPromptRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGenerateAgentFromPrompt {
            bearer,
            prompt: payload.prompt,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        generate_agent_from_prompt_handler,
    ),
    components(
        schemas(GenerateAgentFromPromptRequest, APIError)
    ),
    tags(
        (name = "my_agent", description = "My Agents")
    )
)]
pub struct ToolOfferingsApiDoc;
