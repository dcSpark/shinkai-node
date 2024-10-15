use async_channel::{bounded, Receiver, Sender};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_node::network::Node;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};

#[cfg(test)]
mod tests {
    use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

    use super::*;

    use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
    use tokio::runtime::Runtime;

    // #[test]
    fn test_restore_db() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let node1_identity_name = "@@node1_test.arb-sep-shinkai";
            let node1_subidentity_name = "main";
            let node1_device_name = "node1_device";

            let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
            let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

            let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
                bounded(100);

            let (node1_profile_identity_sk, _) = unsafe_deterministic_signature_keypair(100);
            let (node1_profile_encryption_sk, _) = unsafe_deterministic_encryption_keypair(100);

            let (node1_device_identity_sk, _) = unsafe_deterministic_signature_keypair(200);
            let (node1_device_encryption_sk, _) = unsafe_deterministic_encryption_keypair(200);

            let node1_db_path = "tests/db_for_testing/test".to_string();
            let node1_vector_fs_path = "tests/vector_fs_db_for_testing/test".to_string();

            // Create node1 
            let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
            let node1 = Node::new(
                node1_identity_name.to_string(),
                addr1,
                clone_signature_secret_key(&node1_identity_sk),
                node1_encryption_sk.clone(),
                None,
                None,
                0,
                node1_commands_receiver,
                node1_db_path,
                "".to_string(),
                None,
                true,
                vec![],
                node1_vector_fs_path,
                None,
                None,
                default_embedding_model(),
            supported_embedding_models(),
            None,
            );

            let node1_handler = tokio::spawn(async move {
                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Debug,
                    &format!("Starting Node 1"),
                );
                let _ = node1.await.lock().await.start().await;
            });
            let abort_handler = node1_handler.abort_handle();

            let interactions_handler = tokio::spawn(async move {
                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Debug,
                    &format!("\n\nRegistration of an Admin Profile"),
                );

                {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 1,
                            res: res_sender,
                        })
                        .await
                        .unwrap();
                    let node_last_messages = res_receiver.recv().await.unwrap();
                    let message = node_last_messages[0].clone();
                    let content = message.get_message_content().unwrap();
                    let expected_content = "{\"job_id\":\"jobid_c6ff9307-3965-42e9-9537-4f20f0656af1\",\"content\":\"The document appears to outline a framework for evaluating the planning capabilities of large language models (LLMs) through a benchmark called PlanBench. This benchmark assesses various aspects of reasoning about actions and change, with tasks designed to evaluate different reasoning skills. These tasks involve generating and generalizing plans based on given initial conditions and goals, such as the distribution of objects and their relationships. Examples provided include plans involving loading and flying airplanes, and the craving relationships between objects. The document also references various research papers related to solving math word problems, scaling language models, and automated planning in artificial intelligence\",\"files_inbox\":\"\"}";
                    assert_eq!(content, expected_content);
                }

                abort_handler.abort();
            });

            // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, interactions_handler);

        match result {
            Ok(_) => {},
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    println!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
        });
    }
}
