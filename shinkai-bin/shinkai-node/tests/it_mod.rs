#![recursion_limit = "512"]

mod it {
    mod a3_micropayment_flow_tests;
    mod a4_micropayment_localhost_tests;
    mod cron_job_tests;
    mod db_identity_tests;
    mod db_inbox_tests;
    mod db_job_tests;
    mod db_llm_providers_tests;
    mod db_restore_tests;
    mod job_branchs_retries_tests;
    mod job_code_fork_tests;
    mod job_concurrency_in_seq_tests;
    mod job_fork_messages_tests;
    mod job_image_analysis_tests;
    mod job_manager_concurrency_tests;
    mod job_tree_usage_tests;
    mod model_capabilities_manager_tests;
    mod node_integration_tests;
    mod node_retrying_tests;
    mod node_simple_ux_tests;
    mod performance_tests;
    mod planner_integration_tests;
    mod simple_job_example_tests;
    mod utils;
    mod websocket_tests;

    mod change_nodes_name_tests;
    mod echo_tool_router_key_test;
    mod job_code_duplicate;
    mod native_tool_tests;
    mod tool_config_override_test;
}
