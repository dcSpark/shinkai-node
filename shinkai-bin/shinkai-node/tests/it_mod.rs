#![recursion_limit = "256"]

mod it {
    mod a0_subscription_manager_tests;
    mod agent_integration_tests;
    mod workflow_integration_tests;
    mod cron_job_tests;
    mod crypto_payment_tests;
    mod db_agents_tests;
    mod db_identity_tests;
    mod db_inbox_tests;
    mod db_job_tests;
    mod db_restore_tests;
    mod db_tests;
    mod encrypted_files_tests;
    mod get_onchain_identity_tests;
    mod job_branchs_retries_tests;
    mod job_concurrency_in_seq_tests;
    mod job_image_analysis_tests;
    mod job_manager_concurrency_tests;
    mod job_multi_page_cron_tests;
    mod job_one_page_cron_tests;
    mod model_capabilities_manager_tests;
    mod node_integration_tests;
    mod node_retrying_tests;
    mod node_simple_ux_tests;
    // mod node_toolkit_api_tests;
    mod performance_tests;
    mod planner_integration_tests;
    mod planner_tests;
    mod prompt_tests;
    mod toolkit_tests;
    mod utils;
    mod vector_fs_api_tests;
    mod vector_fs_tests;
    mod subscription_http_upload_tests;
    // mod websocket_tests;
    
    mod z_shinkai_mirror_tests;
    mod tcp_proxy_tests;
    mod change_nodes_name_tests;
}