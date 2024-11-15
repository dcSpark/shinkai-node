use shinkai_http_api::api_v2::{
    api_v2_handlers_ext_agent_offers::ToolOfferingsApiDoc, api_v2_handlers_general::GeneralApiDoc,
    api_v2_handlers_jobs::JobsApiDoc, api_v2_handlers_subscriptions::SubscriptionsApiDoc,
    api_v2_handlers_vecfs::VecFsApiDoc, api_v2_handlers_wallets::WalletApiDoc,
    api_v2_handlers_tools::ToolsApiDoc,
};
use utoipa::OpenApi;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apis = vec![
        ToolOfferingsApiDoc::openapi(),
        GeneralApiDoc::openapi(),
        JobsApiDoc::openapi(),
        SubscriptionsApiDoc::openapi(),
        VecFsApiDoc::openapi(),
        WalletApiDoc::openapi(),
        ToolsApiDoc::openapi(),
    ];

    let schemas_dir = std::path::PathBuf::from("docs/openapi");
    let _ = std::fs::create_dir(&schemas_dir);

    for api in apis {
        let api_doc = api.to_yaml().unwrap();
        std::fs::write(
            schemas_dir.join(format!("{}.yaml", api.tags.unwrap().first().unwrap().name)),
            api_doc,
        )?;
    }

    Ok(())
}
