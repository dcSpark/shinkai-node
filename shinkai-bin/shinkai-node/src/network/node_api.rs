use super::node::NodeCommand;
use super::node_api_handlers::add_agent_handler;
use super::node_api_handlers::add_ollama_models_handler;
use super::node_api_handlers::add_toolkit_handler;
use super::node_api_handlers::api_convert_files_and_save_to_folder_handler;
use super::node_api_handlers::api_my_subscriptions_handler;
use super::node_api_handlers::api_subscription_available_shared_items_handler;
use super::node_api_handlers::api_subscription_available_shared_items_open_handler;
use super::node_api_handlers::api_subscription_create_shareable_folder_handler;
use super::node_api_handlers::api_subscription_unshare_folder_handler;
use super::node_api_handlers::api_subscription_update_shareable_folder_handler;
use super::node_api_handlers::api_vec_fs_copy_folder_handler;
use super::node_api_handlers::api_vec_fs_copy_item_handler;
use super::node_api_handlers::api_vec_fs_create_folder_handler;
use super::node_api_handlers::api_vec_fs_move_folder_handler;
use super::node_api_handlers::api_vec_fs_move_item_handler;
use super::node_api_handlers::api_vec_fs_remove_folder_handler;
use super::node_api_handlers::api_vec_fs_remove_item_handler;
use super::node_api_handlers::api_vec_fs_retrieve_path_minimal_json_handler;
use super::node_api_handlers::api_vec_fs_retrieve_path_simplified_json_handler;
use super::node_api_handlers::api_vec_fs_retrieve_vector_resource_handler;
use super::node_api_handlers::api_vec_fs_retrieve_vector_search_simplified_json_handler;
use super::node_api_handlers::api_vec_fs_search_item_handler;
use super::node_api_handlers::available_llm_providers_handler;
use super::node_api_handlers::change_job_agent_handler;
use super::node_api_handlers::change_nodes_name_handler;
use super::node_api_handlers::create_files_inbox_with_symmetric_key_handler;
use super::node_api_handlers::create_job_handler;
use super::node_api_handlers::create_registration_code_handler;
use super::node_api_handlers::get_all_inboxes_for_profile_handler;
use super::node_api_handlers::get_all_smart_inboxes_for_profile_handler;
use super::node_api_handlers::get_all_subidentities_handler;
use super::node_api_handlers::get_filenames_message_handler;
use super::node_api_handlers::get_last_messages_from_inbox_handler;
use super::node_api_handlers::get_last_messages_from_inbox_with_branches_handler;
use super::node_api_handlers::get_last_unread_messages_from_inbox_handler;
use super::node_api_handlers::get_my_subscribers_handler;
use super::node_api_handlers::get_peers_handler;
use super::node_api_handlers::get_public_key_handler;
use super::node_api_handlers::get_subscription_links_handler;
use super::node_api_handlers::handle_file_upload;
use super::node_api_handlers::identity_name_to_external_profile_data_handler;
use super::node_api_handlers::job_message_handler;
use super::node_api_handlers::mark_as_read_up_to_handler;
use super::node_api_handlers::modify_agent_handler;
use super::node_api_handlers::ping_all_handler;
use super::node_api_handlers::remove_agent_handler;
use super::node_api_handlers::retrieve_vrkai_handler;
use super::node_api_handlers::retrieve_vrpack_handler;
use super::node_api_handlers::scan_ollama_models_handler;
use super::node_api_handlers::send_msg_handler;
use super::node_api_handlers::shinkai_health_handler;
use super::node_api_handlers::subscribe_to_shared_folder_handler;
use super::node_api_handlers::unsubscribe_handler;
use super::node_api_handlers::update_job_to_finished_handler;
use super::node_api_handlers::update_smart_inbox_name_handler;
use super::node_api_handlers::use_registration_code_handler;
use super::node_api_handlers::NameToExternalProfileData;
use async_channel::Sender;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::json;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIAvailableSharedItems;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use warp::Filter;

#[derive(serde::Serialize, Debug, Clone)]
pub struct SendResponseBodyData {
    pub message_id: String,
    pub parent_message_id: Option<String>,
    pub inbox: String,
    pub scheduled_time: String,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct SendResponseBody {
    pub status: String,
    pub message: String,
    pub data: Option<SendResponseBodyData>,
}

#[derive(serde::Serialize)]
pub struct GetPublicKeysResponse {
    pub signature_public_key: String,
    pub encryption_public_key: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct APIError {
    pub code: u16,
    pub error: String,
    pub message: String,
}

impl APIError {
    fn new(code: StatusCode, error: &str, message: &str) -> Self {
        Self {
            code: code.as_u16(),
            error: error.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<&str> for APIError {
    fn from(error: &str) -> Self {
        APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: error.to_string(),
        }
    }
}

impl From<async_channel::SendError<NodeCommand>> for APIError {
    fn from(error: async_channel::SendError<NodeCommand>) -> Self {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed with error: {}", error),
        }
    }
}

impl From<String> for APIError {
    fn from(error: String) -> Self {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: error,
        }
    }
}

impl warp::reject::Reject for APIError {}

pub async fn run_api(
    node_commands_sender: Sender<NodeCommand>,
    address: SocketAddr,
    node_name: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    shinkai_log(
        ShinkaiLogOption::Api,
        ShinkaiLogLevel::Info,
        &format!("Starting Node API server at: {}", &address),
    );

    let log = warp::log::custom(|info| {
        shinkai_log(
            ShinkaiLogOption::Api,
            ShinkaiLogLevel::Debug,
            &format!(
                "ip: {:?}, method: {:?}, path: {:?}, status: {:?}, elapsed: {:?}",
                info.remote_addr(),
                info.method(),
                info.path(),
                info.status(),
                info.elapsed(),
            ),
        );
    });

    let ping_all = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "ping_all")
            .and(warp::post())
            .and_then(move || ping_all_handler(node_commands_sender.clone()))
    };

    // POST v1/send
    let send_msg = {
        let node_commands_sender = node_commands_sender.clone();
        warp::post()
            .and(warp::path("v1"))
            .and(warp::path("send"))
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| send_msg_handler(node_commands_sender.clone(), message))
    };

    // GET v1/get_peers
    let get_peers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_peers")
            .and(warp::get())
            .and_then(move || get_peers_handler(node_commands_sender.clone()))
    };

    // POST v1/identity_name_to_external_profile_data
    let identity_name_to_external_profile_data = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "identity_name_to_external_profile_data")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: NameToExternalProfileData| {
                identity_name_to_external_profile_data_handler(node_commands_sender.clone(), body)
            })
    };

    // GET v1/get_public_keys
    let get_public_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_public_keys")
            .and(warp::get())
            .and_then(move || get_public_key_handler(node_commands_sender.clone()))
    };

    // POST v1/add_toolkit
    let add_toolkit = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "add_toolkit")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_toolkit_handler(node_commands_sender.clone(), message))
    };

    // POST v1/vec_fs/retrieve_path_simplified_json
    let api_vec_fs_retrieve_path_simplified_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "retrieve_path_simplified_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_path_simplified_json_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/retrieve_path_minimal_json
    let api_vec_fs_retrieve_path_minimal_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "retrieve_path_minimal_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_path_minimal_json_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/retrieve_vector_search_simplified_json
    let api_vec_fs_retrieve_vector_search_simplified_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "retrieve_vector_search_simplified_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_vector_search_simplified_json_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/search_items
    let api_vec_fs_search_items = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "search_items")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_search_item_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/create_folder
    let api_vec_fs_create_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "create_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_create_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/move_folder
    let api_vec_fs_move_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "move_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_move_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/copy_folder
    let api_vec_fs_copy_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "copy_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_copy_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/remove_folder
    let api_vec_fs_remove_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "remove_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_remove_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/move_item
    let api_vec_fs_move_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "move_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_move_item_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/copy_item
    let api_vec_fs_copy_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "copy_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_copy_item_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/remove_item
    let api_vec_fs_remove_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "remove_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_remove_item_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/retrieve_vector_resource
    let api_convert_files_and_save_to_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "convert_files_and_save_to_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_convert_files_and_save_to_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/vec_fs/retrieve_vector_resource
    let api_vec_fs_retrieve_vector_resource = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "vec_fs" / "retrieve_vector_resource")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_vector_resource_handler(node_commands_sender.clone(), message)
            })
    };

    // GET v1/shinkai_health
    let shinkai_health = {
        let node_name = node_name.clone();
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "shinkai_health")
            .and(warp::get())
            .and_then(move || shinkai_health_handler(node_commands_sender.clone(), node_name.clone()))
    };

    // TODO: Implement. Admin Only
    // // POST v1/last_messages?limit={number}&offset={key}
    // let get_last_messages = {
    //     let node_commands_sender = node_commands_sender.clone();
    //     warp::path!("v1" / "last_messages_from_inbox")
    //         .and(warp::post())
    //         .and(warp::body::json::<ShinkaiMessage>())
    //         .and_then(move |message: ShinkaiMessage| {
    //             get_last_messages_handler(node_commands_sender.clone(), message)
    //         })
    // };

    // POST v1/available_agents
    let available_llm_providers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "available_agents")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| available_llm_providers_handler(node_commands_sender.clone(), message))
    };

    // POST v1/add_agent
    let add_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "add_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_agent_handler(node_commands_sender.clone(), message))
    };

    // POST v1/modify_agent
    let modify_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "modify_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| modify_agent_handler(node_commands_sender.clone(), message))
    };

    // POST v1/remove_agent
    let remove_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "remove_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| remove_agent_handler(node_commands_sender.clone(), message))
    };

    // POST v1/last_messages_from_inbox?limit={number}&offset={key}
    let get_last_messages_from_inbox = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/last_unread_messages?limit={number}&offset={key}
    let get_last_unread_messages = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_unread_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_unread_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/get_all_inboxes_for_profile_handler
    let get_all_inboxes_for_profile = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_all_inboxes_for_profile")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_all_inboxes_for_profile_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/get_all_smart_inboxes_for_profile_handler
    let get_all_smart_inboxes_for_profile = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_all_smart_inboxes_for_profile")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_all_smart_inboxes_for_profile_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/update_smart_inbox_name_handler
    let update_smart_inbox_name = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "update_smart_inbox_name")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                update_smart_inbox_name_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/create_job
    let create_job = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_job")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| create_job_handler(node_commands_sender.clone(), message))
    };

    // POST v1/job_message
    let job_message = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "job_message")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| job_message_handler(node_commands_sender.clone(), message))
    };

    // POST v1/get_filenames_for_file_inbox
    let get_filenames = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_filenames_for_file_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_filenames_message_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/mark_as_read_up_to
    let mark_as_read_up_to = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "mark_as_read_up_to")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| mark_as_read_up_to_handler(node_commands_sender.clone(), message))
    };

    // POST v1/create_registration_code
    let create_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                create_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/use_registration_code
    let use_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "use_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                use_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/change_nodes_name
    let change_nodes_name = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "change_nodes_name")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| change_nodes_name_handler(node_commands_sender.clone(), message))
    };

    // GET v1/get_all_subidentities
    let get_all_subidentities = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_all_subidentities")
            .and(warp::get())
            .and_then(move || get_all_subidentities_handler(node_commands_sender.clone()))
    };

    // POST v1/last_messages_from_inbox_with_branches
    let get_last_messages_from_inbox_with_branches = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_messages_from_inbox_with_branches")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_messages_from_inbox_with_branches_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/create_files_inbox_with_symmetric_key
    let create_files_inbox_with_symmetric_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_files_inbox_with_symmetric_key")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                create_files_inbox_with_symmetric_key_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/add_file_to_inbox_with_symmetric_key/{string1}/{string2}
    let add_file_to_inbox_with_symmetric_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "add_file_to_inbox_with_symmetric_key" / String / String)
            .and(warp::post())
            .and(warp::body::content_length_limit(1024 * 1024 * 200)) // 200MB
            .and(warp::multipart::form().max_length(1024 * 1024 * 200))
            .and_then(
                move |string1: String, string2: String, form: warp::multipart::FormData| {
                    handle_file_upload(node_commands_sender.clone(), string1, string2, form)
                },
            )
    };

    // POST v1/update_job_to_finished
    let update_job_to_finished = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "update_job_to_finished")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                update_job_to_finished_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/available_shared_items
    let api_available_shared_items = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "available_shared_items")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_available_shared_items_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/available_shared_items_open
    let api_available_shared_items_open = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "available_shared_items_open")
            .and(warp::post())
            .and(warp::body::json::<APIAvailableSharedItems>())
            .and_then(move |message: APIAvailableSharedItems| {
                api_subscription_available_shared_items_open_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/my_subscriptions
    let my_subscriptions = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "my_subscriptions")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_my_subscriptions_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/create_shareable_folder
    let api_create_shareable_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_shareable_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_create_shareable_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/subscribe_to_shared_folder
    let subscribe_to_shared_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "subscribe_to_shared_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                subscribe_to_shared_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/update_shareable_folder
    let api_update_shareable_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "update_shareable_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_update_shareable_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/unshare_folder
    let api_unshare_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "unshare_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_unshare_folder_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/unsubscribe
    let unsubscribe = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "unsubscribe")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| unsubscribe_handler(node_commands_sender.clone(), message))
    };

    // POST v1/get_my_subscribers
    let get_my_subscribers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "my_subscribers")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| get_my_subscribers_handler(node_commands_sender.clone(), message))
    };

    // POST v1/retrieve_vrkai
    let retrieve_vrkai = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "retrieve_vrkai")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| retrieve_vrkai_handler(node_commands_sender.clone(), message))
    };

    // POST v1/retrieve_vrpack
    let retrieve_vrpack = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "retrieve_vrpack")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| retrieve_vrpack_handler(node_commands_sender.clone(), message))
    };

    // POST v1/local_scan_ollama_models
    let local_scan_ollama_models = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "scan_ollama_models")
            .and(warp::post()) // Corrected from .and(warp::get()) to match the handler's expected method
            .and(warp::body::json::<ShinkaiMessage>()) // Ensure the body is deserialized into a ShinkaiMessage
            .and_then(move |message: ShinkaiMessage| scan_ollama_models_handler(node_commands_sender.clone(), message))
        // Corrected handler name and added message parameter
    };

    // POST v1/add_ollama_models
    let add_ollama_models = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "add_ollama_models")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>()) // Updated to accept ShinkaiMessage instead of Vec<String>
            .and_then(move |message: ShinkaiMessage| add_ollama_models_handler(node_commands_sender.clone(), message))
        // Corrected to pass ShinkaiMessage to the handler
    };

    // GET v1/subscriptions/{subs_key}/links
    let get_subscription_links = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "subscriptions" / String / "links")
            .and(warp::get())
            .and_then(move |subscription_id: String| {
                get_subscription_links_handler(node_commands_sender.clone(), subscription_id)
            })
    };

    // POST v1/change_job_agent
    let change_job_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "change_job_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| change_job_agent_handler(node_commands_sender.clone(), message))
    };

    let cors = warp::cors() // build the CORS filter
        .allow_any_origin() // allow requests from any origin
        .allow_methods(vec!["GET", "POST", "OPTIONS"]) // allow GET, POST, and OPTIONS methods
        .allow_headers(vec!["Content-Type", "Authorization"]); // allow the Content-Type and Authorization headers

    let routes = ping_all
        .or(send_msg)
        .or(get_peers)
        .or(identity_name_to_external_profile_data)
        .or(get_public_key)
        .or(get_all_inboxes_for_profile)
        .or(get_all_smart_inboxes_for_profile)
        .or(update_smart_inbox_name)
        .or(available_llm_providers)
        .or(add_agent)
        .or(remove_agent)
        .or(modify_agent)
        .or(get_last_messages_from_inbox)
        .or(get_last_unread_messages)
        .or(create_job)
        .or(job_message)
        .or(mark_as_read_up_to)
        .or(create_registration_code)
        .or(use_registration_code)
        .or(get_all_subidentities)
        .or(shinkai_health)
        .or(create_files_inbox_with_symmetric_key)
        .or(add_file_to_inbox_with_symmetric_key)
        .or(get_filenames)
        .or(update_job_to_finished)
        .or(add_toolkit)
        .or(change_nodes_name)
        .or(get_last_messages_from_inbox_with_branches)
        .or(api_vec_fs_retrieve_path_simplified_json)
        .or(api_vec_fs_retrieve_path_minimal_json)
        .or(api_vec_fs_retrieve_vector_search_simplified_json)
        .or(api_vec_fs_search_items)
        .or(api_vec_fs_create_folder)
        .or(api_vec_fs_move_item)
        .or(api_vec_fs_copy_item)
        .or(api_vec_fs_remove_item)
        .or(api_vec_fs_move_folder)
        .or(api_vec_fs_copy_folder)
        .or(api_vec_fs_remove_folder)
        .or(api_vec_fs_retrieve_vector_resource)
        .or(api_convert_files_and_save_to_folder)
        .or(local_scan_ollama_models)
        .or(add_ollama_models)
        .or(api_available_shared_items)
        .or(api_available_shared_items_open)
        .or(api_create_shareable_folder)
        .or(api_update_shareable_folder)
        .or(api_unshare_folder)
        .or(my_subscriptions)
        .or(get_my_subscribers)
        .or(subscribe_to_shared_folder)
        .or(unsubscribe)
        .or(retrieve_vrkai)
        .or(retrieve_vrpack)
        .or(get_subscription_links)
        .or(change_job_agent)
        .recover(handle_rejection)
        .with(log)
        .with(cors);

    // Attempt to bind to the address before serving
    let try_bind = TcpListener::bind(&address).await;

    match try_bind {
        Ok(_) => {
            drop(try_bind);
            warp::serve(routes).run(address).await;
            Ok(())
        }
        Err(e) => {
            // If binding fails, return an error
            Err(Box::new(e))
        }
    }
}

pub async fn handle_node_command<T, U, V>(
    node_commands_sender: Sender<NodeCommand>,
    message: V,
    command: T,
) -> Result<impl warp::Reply, warp::reject::Rejection>
where
    T: FnOnce(Sender<NodeCommand>, V, Sender<Result<U, APIError>>) -> NodeCommand,
    U: Serialize,
    V: Serialize,
{
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .clone()
        .send(command(node_commands_sender, message, res_sender))
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(message) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"status": "success", "data": message})),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"status": "error", "error": error.message})),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(api_error) = err.find::<APIError>() {
        let json = warp::reply::json(api_error);
        Ok(warp::reply::with_status(
            json,
            StatusCode::from_u16(api_error.code).unwrap(),
        ))
    } else if err.is_not_found() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Please check your URL.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::NOT_FOUND))
    } else if err.find::<warp::filters::body::BodyDeserializeError>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::BAD_REQUEST,
            "Invalid Body",
            "Please check your JSON body.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
            "Please check your request method.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::METHOD_NOT_ALLOWED))
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Payload Too Large",
            "The request payload is too large.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::PAYLOAD_TOO_LARGE))
    } else if err.find::<warp::reject::InvalidQuery>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::BAD_REQUEST,
            "Invalid Query",
            "The request query string is invalid.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
    } else {
        // Unexpected error, we don't want to expose anything to the user.
        let json = warp::reply::json(&APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "An unexpected error occurred. Please try again.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
    }
}
