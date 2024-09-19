use std::sync::Arc;

use crate::network::{node_api_router::APIError, node_error::NodeError, Node};

use async_channel::Sender;
use reqwest::StatusCode;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

impl Node {
    pub async fn api_private_devops_cron_list(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Call the get_all_cron_tasks_from_all_profiles function
        match db.get_all_cron_tasks_from_all_profiles(node_name.clone()) {
            Ok(tasks) => {
                // If everything went well, send the tasks back as a JSON string
                let tasks_json = serde_json::to_string(&tasks).unwrap();
                let _ = res.send(Ok(tasks_json)).await;
                Ok(())
            }
            Err(err) => {
                // If there was an error, send the error message
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }
}
