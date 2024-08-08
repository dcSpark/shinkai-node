use async_channel::Sender;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIAvailableSharedItems;
use warp::Filter;

use crate::network::node_commands::NodeCommand;
use super::api_v1_handlers::add_agent_handler;
use super::api_v1_handlers::add_ollama_models_handler;
use super::api_v1_handlers::add_row_handler;
use super::api_v1_handlers::add_toolkit_handler;
use super::api_v1_handlers::add_workflow_handler;
use super::api_v1_handlers::api_convert_files_and_save_to_folder_handler;
use super::api_v1_handlers::api_my_subscriptions_handler;
use super::api_v1_handlers::api_subscription_available_shared_items_handler;
use super::api_v1_handlers::api_subscription_available_shared_items_open_handler;
use super::api_v1_handlers::api_subscription_create_shareable_folder_handler;
use super::api_v1_handlers::api_subscription_unshare_folder_handler;
use super::api_v1_handlers::api_subscription_update_shareable_folder_handler;
use super::api_v1_handlers::api_update_default_embedding_model_handler;
use super::api_v1_handlers::api_update_supported_embedding_models_handler;
use super::api_v1_handlers::api_vec_fs_copy_folder_handler;
use super::api_v1_handlers::api_vec_fs_copy_item_handler;
use super::api_v1_handlers::api_vec_fs_create_folder_handler;
use super::api_v1_handlers::api_vec_fs_move_folder_handler;
use super::api_v1_handlers::api_vec_fs_move_item_handler;
use super::api_v1_handlers::api_vec_fs_remove_folder_handler;
use super::api_v1_handlers::api_vec_fs_remove_item_handler;
use super::api_v1_handlers::api_vec_fs_retrieve_path_minimal_json_handler;
use super::api_v1_handlers::api_vec_fs_retrieve_path_simplified_json_handler;
use super::api_v1_handlers::api_vec_fs_retrieve_vector_resource_handler;
use super::api_v1_handlers::api_vec_fs_retrieve_vector_search_simplified_json_handler;
use super::api_v1_handlers::api_vec_fs_search_item_handler;
use super::api_v1_handlers::available_llm_providers_handler;
use super::api_v1_handlers::change_job_agent_handler;
use super::api_v1_handlers::change_nodes_name_handler;
use super::api_v1_handlers::create_files_inbox_with_symmetric_key_handler;
use super::api_v1_handlers::create_job_handler;
use super::api_v1_handlers::create_registration_code_handler;
use super::api_v1_handlers::create_sheet_handler;
use super::api_v1_handlers::delete_workflow_handler;
use super::api_v1_handlers::get_all_inboxes_for_profile_handler;
use super::api_v1_handlers::get_all_smart_inboxes_for_profile_handler;
use super::api_v1_handlers::get_all_subidentities_handler;
use super::api_v1_handlers::get_filenames_message_handler;
use super::api_v1_handlers::get_last_messages_from_inbox_handler;
use super::api_v1_handlers::get_last_messages_from_inbox_with_branches_handler;
use super::api_v1_handlers::get_last_notifications_handler;
use super::api_v1_handlers::get_last_unread_messages_from_inbox_handler;
use super::api_v1_handlers::get_local_processing_preference_handler;
use super::api_v1_handlers::get_my_subscribers_handler;
use super::api_v1_handlers::get_notifications_before_timestamp_handler;
use super::api_v1_handlers::get_public_key_handler;
use super::api_v1_handlers::get_sheet_handler;
use super::api_v1_handlers::get_subscription_links_handler;
use super::api_v1_handlers::get_workflow_info_handler;
use super::api_v1_handlers::handle_file_upload;
use super::api_v1_handlers::identity_name_to_external_profile_data_handler;
use super::api_v1_handlers::job_message_handler;
use super::api_v1_handlers::list_all_workflows_handler;
use super::api_v1_handlers::mark_as_read_up_to_handler;
use super::api_v1_handlers::modify_agent_handler;
use super::api_v1_handlers::ping_all_handler;
use super::api_v1_handlers::remove_agent_handler;
use super::api_v1_handlers::remove_column_handler;
use super::api_v1_handlers::remove_row_handler;
use super::api_v1_handlers::remove_sheet_handler;
use super::api_v1_handlers::retrieve_vrkai_handler;
use super::api_v1_handlers::retrieve_vrpack_handler;
use super::api_v1_handlers::scan_ollama_models_handler;
use super::api_v1_handlers::search_workflows_handler;
use super::api_v1_handlers::send_msg_handler;
use super::api_v1_handlers::set_cell_value_handler;
use super::api_v1_handlers::set_column_handler;
use super::api_v1_handlers::shinkai_health_handler;
use super::api_v1_handlers::subscribe_to_shared_folder_handler;
use super::api_v1_handlers::unsubscribe_handler;
use super::api_v1_handlers::update_job_to_finished_handler;
use super::api_v1_handlers::update_local_processing_preference_handler;
use super::api_v1_handlers::update_smart_inbox_name_handler;
use super::api_v1_handlers::update_workflow_handler;
use super::api_v1_handlers::use_registration_code_handler;
use super::api_v1_handlers::user_sheets_handler;
use super::api_v1_handlers::NameToExternalProfileData;

pub fn v1_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let ping_all = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("ping_all")
            .and(warp::post())
            .and_then(move || ping_all_handler(node_commands_sender.clone()))
    };

    let send_msg = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("send")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| send_msg_handler(node_commands_sender.clone(), message))
    };


    let identity_name_to_external_profile_data = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("identity_name_to_external_profile_data")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: NameToExternalProfileData| {
                identity_name_to_external_profile_data_handler(node_commands_sender.clone(), body)
            })
    };

    let get_public_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_public_keys")
            .and(warp::get())
            .and_then(move || get_public_key_handler(node_commands_sender.clone()))
    };

    let add_toolkit = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_toolkit")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_toolkit_handler(node_commands_sender.clone(), message))
    };

    let api_vec_fs_retrieve_path_simplified_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "retrieve_path_simplified_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_path_simplified_json_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_retrieve_path_minimal_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "retrieve_path_minimal_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_path_minimal_json_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_retrieve_vector_search_simplified_json = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "retrieve_vector_search_simplified_json")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_vector_search_simplified_json_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_search_items = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "search_items")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_search_item_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_create_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "create_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_create_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_move_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "move_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_move_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_copy_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "copy_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_copy_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_remove_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "remove_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_remove_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_move_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "move_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_move_item_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_copy_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "copy_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_copy_item_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_remove_item = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "remove_item")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_remove_item_handler(node_commands_sender.clone(), message)
            })
    };

    let api_convert_files_and_save_to_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "convert_files_and_save_to_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_convert_files_and_save_to_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_vec_fs_retrieve_vector_resource = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("vec_fs" / "retrieve_vector_resource")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_vec_fs_retrieve_vector_resource_handler(node_commands_sender.clone(), message)
            })
    };

    let shinkai_health = {
        let node_name = node_name.clone();
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("shinkai_health")
            .and(warp::get())
            .and_then(move || shinkai_health_handler(node_commands_sender.clone(), node_name.clone()))
    };

    let available_llm_providers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("available_agents")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                available_llm_providers_handler(node_commands_sender.clone(), message)
            })
    };

    let add_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_agent_handler(node_commands_sender.clone(), message))
    };

    let modify_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("modify_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| modify_agent_handler(node_commands_sender.clone(), message))
    };

    let remove_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("remove_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| remove_agent_handler(node_commands_sender.clone(), message))
    };

    let get_last_messages_from_inbox = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("last_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    let get_last_unread_messages = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("last_unread_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_unread_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    let get_all_inboxes_for_profile = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_all_inboxes_for_profile")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_all_inboxes_for_profile_handler(node_commands_sender.clone(), message)
            })
    };

    let get_all_smart_inboxes_for_profile = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_all_smart_inboxes_for_profile")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_all_smart_inboxes_for_profile_handler(node_commands_sender.clone(), message)
            })
    };

    let update_smart_inbox_name = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_smart_inbox_name")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                update_smart_inbox_name_handler(node_commands_sender.clone(), message)
            })
    };

    let create_job = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("create_job")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| create_job_handler(node_commands_sender.clone(), message))
    };

    let job_message = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("job_message")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| job_message_handler(node_commands_sender.clone(), message))
    };

    let get_filenames = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_filenames_for_file_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_filenames_message_handler(node_commands_sender.clone(), message)
            })
    };

    let mark_as_read_up_to = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("mark_as_read_up_to")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| mark_as_read_up_to_handler(node_commands_sender.clone(), message))
    };

    let create_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("create_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                create_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    let use_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("use_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                use_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    let change_nodes_name = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("change_nodes_name")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| change_nodes_name_handler(node_commands_sender.clone(), message))
    };

    let get_all_subidentities = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_all_subidentities")
            .and(warp::get())
            .and_then(move || get_all_subidentities_handler(node_commands_sender.clone()))
    };

    let get_last_messages_from_inbox_with_branches = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("last_messages_from_inbox_with_branches")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_messages_from_inbox_with_branches_handler(node_commands_sender.clone(), message)
            })
    };

    let create_files_inbox_with_symmetric_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("create_files_inbox_with_symmetric_key")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                create_files_inbox_with_symmetric_key_handler(node_commands_sender.clone(), message)
            })
    };

    let add_file_to_inbox_with_symmetric_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_file_to_inbox_with_symmetric_key" / String / String)
            .and(warp::post())
            .and(warp::body::content_length_limit(1024 * 1024 * 200)) // 200MB
            .and(warp::multipart::form().max_length(1024 * 1024 * 200))
            .and_then(
                move |string1: String, string2: String, form: warp::multipart::FormData| {
                    handle_file_upload(node_commands_sender.clone(), string1, string2, form)
                },
            )
    };

    let update_job_to_finished = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_job_to_finished")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                update_job_to_finished_handler(node_commands_sender.clone(), message)
            })
    };

    let api_available_shared_items = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("available_shared_items")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_available_shared_items_handler(node_commands_sender.clone(), message)
            })
    };

    let api_available_shared_items_open = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("available_shared_items_open")
            .and(warp::post())
            .and(warp::body::json::<APIAvailableSharedItems>())
            .and_then(move |message: APIAvailableSharedItems| {
                api_subscription_available_shared_items_open_handler(node_commands_sender.clone(), message)
            })
    };

    let my_subscriptions = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("my_subscriptions")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_my_subscriptions_handler(node_commands_sender.clone(), message)
            })
    };

    let api_create_shareable_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("create_shareable_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_create_shareable_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let subscribe_to_shared_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("subscribe_to_shared_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                subscribe_to_shared_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_update_shareable_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_shareable_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_update_shareable_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let api_unshare_folder = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("unshare_folder")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_subscription_unshare_folder_handler(node_commands_sender.clone(), message)
            })
    };

    let unsubscribe = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("unsubscribe")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| unsubscribe_handler(node_commands_sender.clone(), message))
    };

    let get_my_subscribers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("my_subscribers")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| get_my_subscribers_handler(node_commands_sender.clone(), message))
    };

    let retrieve_vrkai = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("retrieve_vrkai")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| retrieve_vrkai_handler(node_commands_sender.clone(), message))
    };

    let retrieve_vrpack = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("retrieve_vrpack")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| retrieve_vrpack_handler(node_commands_sender.clone(), message))
    };

    let local_scan_ollama_models = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("scan_ollama_models")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| scan_ollama_models_handler(node_commands_sender.clone(), message))
    };

    let add_ollama_models = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_ollama_models")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_ollama_models_handler(node_commands_sender.clone(), message))
    };

    let get_subscription_links = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("subscriptions" / String / "links")
            .and(warp::get())
            .and_then(move |subscription_id: String| {
                get_subscription_links_handler(node_commands_sender.clone(), subscription_id)
            })
    };

    let change_job_agent = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("change_job_agent")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| change_job_agent_handler(node_commands_sender.clone(), message))
    };

    let get_last_notifications = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_last_notifications")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_notifications_handler(node_commands_sender.clone(), message)
            })
    };

    let get_notifications_before_timestamp = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_notifications_before_timestamp")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_notifications_before_timestamp_handler(node_commands_sender.clone(), message)
            })
    };

    let get_local_processing_preference = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_local_processing_preference")
            .and(warp::get())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_local_processing_preference_handler(node_commands_sender.clone(), message)
            })
    };

    let update_local_processing_preference = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_local_processing_preference")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                update_local_processing_preference_handler(node_commands_sender.clone(), message)
            })
    };

    let search_workflows = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("search_workflows")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| search_workflows_handler(node_commands_sender.clone(), message))
    };

    let add_workflow = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_workflow")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_workflow_handler(node_commands_sender.clone(), message))
    };

    let update_workflow = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_workflow")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| update_workflow_handler(node_commands_sender.clone(), message))
    };

    let delete_workflow = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("delete_workflow")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| delete_workflow_handler(node_commands_sender.clone(), message))
    };

    let get_workflow_info = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_workflow_info")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| get_workflow_info_handler(node_commands_sender.clone(), message))
    };

    let list_all_workflows = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("list_all_workflows")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| list_all_workflows_handler(node_commands_sender.clone(), message))
    };

    let set_column = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("set_column")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| set_column_handler(node_commands_sender.clone(), message))
    };

    let remove_column = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("remove_column")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| remove_column_handler(node_commands_sender.clone(), message))
    };

    let add_row = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("add_rows")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| add_row_handler(node_commands_sender.clone(), message))
    };

    let remove_row = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("remove_rows")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| remove_row_handler(node_commands_sender.clone(), message))
    };

    let user_sheets = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("user_sheets")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| user_sheets_handler(node_commands_sender.clone(), message))
    };

    let create_sheet = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("create_sheet")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| create_sheet_handler(node_commands_sender.clone(), message))
    };

    let remove_sheet = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("remove_sheet")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| remove_sheet_handler(node_commands_sender.clone(), message))
    };

    let get_sheet = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("get_sheet")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| get_sheet_handler(node_commands_sender.clone(), message))
    };

    let set_cell_value = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("set_cell_value")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| set_cell_value_handler(node_commands_sender.clone(), message))
    };

    let api_update_default_embedding_model = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_default_embedding_model")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_update_default_embedding_model_handler(node_commands_sender.clone(), message)
            })
    };

    let api_update_supported_embedding_models = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("update_supported_embedding_models")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                api_update_supported_embedding_models_handler(node_commands_sender.clone(), message)
            })
    };

    ping_all
        .or(send_msg)
        .or(identity_name_to_external_profile_data)
        .or(get_public_key)
        .or(add_toolkit)
        .or(api_vec_fs_retrieve_path_simplified_json)
        .or(api_vec_fs_retrieve_path_minimal_json)
        .or(api_vec_fs_retrieve_vector_search_simplified_json)
        .or(api_vec_fs_search_items)
        .or(api_vec_fs_create_folder)
        .or(api_vec_fs_move_folder)
        .or(api_vec_fs_copy_folder)
        .or(api_vec_fs_remove_folder)
        .or(api_vec_fs_move_item)
        .or(api_vec_fs_copy_item)
        .or(api_vec_fs_remove_item)
        .or(api_convert_files_and_save_to_folder)
        .or(api_vec_fs_retrieve_vector_resource)
        .or(shinkai_health)
        .or(available_llm_providers)
        .or(add_agent)
        .or(modify_agent)
        .or(remove_agent)
        .or(get_last_messages_from_inbox)
        .or(get_last_unread_messages)
        .or(get_all_inboxes_for_profile)
        .or(get_all_smart_inboxes_for_profile)
        .or(update_smart_inbox_name)
        .or(create_job)
        .or(job_message)
        .or(get_filenames)
        .or(mark_as_read_up_to)
        .or(create_registration_code)
        .or(use_registration_code)
        .or(change_nodes_name)
        .or(get_all_subidentities)
        .or(get_last_messages_from_inbox_with_branches)
        .or(create_files_inbox_with_symmetric_key)
        .or(add_file_to_inbox_with_symmetric_key)
        .or(update_job_to_finished)
        .or(api_available_shared_items)
        .or(api_available_shared_items_open)
        .or(my_subscriptions)
        .or(api_create_shareable_folder)
        .or(subscribe_to_shared_folder)
        .or(api_update_shareable_folder)
        .or(api_unshare_folder)
        .or(unsubscribe)
        .or(get_my_subscribers)
        .or(retrieve_vrkai)
        .or(retrieve_vrpack)
        .or(local_scan_ollama_models)
        .or(add_ollama_models)
        .or(get_subscription_links)
        .or(change_job_agent)
        .or(get_last_notifications)
        .or(get_notifications_before_timestamp)
        .or(get_local_processing_preference)
        .or(update_local_processing_preference)
        .or(search_workflows)
        .or(add_workflow)
        .or(update_workflow)
        .or(delete_workflow)
        .or(get_workflow_info)
        .or(list_all_workflows)
        .or(set_column)
        .or(remove_column)
        .or(add_row)
        .or(remove_row)
        .or(user_sheets)
        .or(get_sheet)
        .or(create_sheet)
        .or(remove_sheet)
        .or(set_cell_value)
        .or(api_update_default_embedding_model)
        .or(api_update_supported_embedding_models)
}
