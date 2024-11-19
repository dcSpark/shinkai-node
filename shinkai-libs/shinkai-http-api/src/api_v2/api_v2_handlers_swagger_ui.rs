use std::sync::Arc;

use utoipa::OpenApi;
use utoipa_swagger_ui::Config;
use warp::{
    filters::path::{FullPath, Tail},
    http::Uri,
    hyper::{Response, StatusCode},
    reject::Rejection,
    reply::Reply,
    Filter,
};

use super::{
    api_v2_handlers_ext_agent_offers::ToolOfferingsApiDoc, api_v2_handlers_general::GeneralApiDoc, api_v2_handlers_jobs::JobsApiDoc, api_v2_handlers_subscriptions::SubscriptionsApiDoc, api_v2_handlers_tools::ToolsApiDoc, api_v2_handlers_vecfs::VecFsApiDoc, api_v2_handlers_wallets::WalletApiDoc
};

pub fn swagger_ui_routes() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let config = Arc::new(Config::new([
        "/v2/openapi/general.json",
        "/v2/openapi/jobs.json",
        "/v2/openapi/subscriptions.json",
        "/v2/openapi/vecfs.json",
        "/v2/openapi/wallet.json",
        "/v2/openapi/tools.json",
        "/v2/openapi/ext_agent_offers.json",
    ]));

    let general_schema_route = warp::path!("openapi" / "general.json")
        .and(warp::get())
        .map(|| warp::reply::json(&GeneralApiDoc::openapi()));

    let jobs_schema_route = warp::path!("openapi" / "jobs.json")
        .and(warp::get())
        .map(|| warp::reply::json(&JobsApiDoc::openapi()));

    let subscriptions_schema_route = warp::path!("openapi" / "subscriptions.json")
        .and(warp::get())
        .map(|| warp::reply::json(&SubscriptionsApiDoc::openapi()));

    let vecfs_schema_route = warp::path!("openapi" / "vecfs.json")
        .and(warp::get())
        .map(|| warp::reply::json(&VecFsApiDoc::openapi()));

    let wallet_schema_route = warp::path!("openapi" / "wallet.json")
        .and(warp::get())
        .map(|| warp::reply::json(&WalletApiDoc::openapi()));

    let tools_schema_route = warp::path!("openapi" / "tools.json")
        .and(warp::get())
        .map(|| warp::reply::json(&ToolsApiDoc::openapi()));

    let ext_agent_offers_schema_route = warp::path!("openapi" / "ext_agent_offers.json")
        .and(warp::get())
        .map(|| warp::reply::json(&ToolOfferingsApiDoc::openapi()));

    let swagger_ui = warp::path("swagger-ui")
        .and(warp::get())
        .and(warp::path::full())
        .and(warp::path::tail())
        .and(warp::any().map(move || config.clone()))
        .and_then(serve_swagger);

    general_schema_route
        .or(jobs_schema_route)
        .or(subscriptions_schema_route)
        .or(vecfs_schema_route)
        .or(wallet_schema_route)
        .or(tools_schema_route)
        .or(ext_agent_offers_schema_route)
        .or(swagger_ui)
}

async fn serve_swagger(
    full_path: FullPath,
    tail: Tail,
    config: Arc<Config<'static>>,
) -> Result<Box<dyn Reply + 'static>, Rejection> {
    if full_path.as_str() == "/v2/swagger-ui" {
        return Ok(Box::new(warp::redirect::found(Uri::from_static("/v2/swagger-ui/"))));
    }

    let path = tail.as_str();
    match utoipa_swagger_ui::serve(path, config) {
        Ok(file) => {
            if let Some(file) = file {
                Ok(Box::new(
                    Response::builder()
                        .header("Content-Type", file.content_type)
                        .body(file.bytes),
                ))
            } else {
                Ok(Box::new(StatusCode::NOT_FOUND))
            }
        }
        Err(error) => Ok(Box::new(
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(error.to_string()),
        )),
    }
}
