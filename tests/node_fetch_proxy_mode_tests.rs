mod utils;
use shinkai_node::{network::node::NodeProxyMode, utils::environment::fetch_node_proxy_mode};

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    use super::*;
    use std::env;

    #[test]
    fn test_fetch_node_proxy_mode() {
        // Set the environment variables
        env::set_var("NODE_PROXY_MODE", "IsProxy");
        env::set_var("ALLOW_NEW_IDENTITIES", "true");
        env::set_var("PROXY_IDENTITIES_CSV_PATH", "./files/proxy_identities_test.csv");

        // Call the function
        let proxy_mode = fetch_node_proxy_mode();

        // Check the result
        match proxy_mode {
            NodeProxyMode::IsProxy(conf) => {
                assert!(conf.allow_new_identities);
                assert_eq!(conf.proxy_node_identities.len(), 3);
                assert!(conf.proxy_node_identities.contains_key("identity1"));
                assert!(conf.proxy_node_identities.contains_key("identity2"));
                assert!(conf.proxy_node_identities.contains_key("identity3"));
            }
            _ => panic!("Expected IsProxy mode"),
        }
    }

    #[test]
    fn test_fetch_node_is_proxied_mode() {
        // Set the environment variables
        env::set_var("NODE_PROXY_MODE", "IsProxied");
        env::set_var("PROXY_API_PEER", "127.0.0.1:9550");
        env::set_var("PROXY_TCP_PEER", "127.0.0.1:9552");
        env::set_var("GLOBAL_IDENTITY_NAME", "@@node1.shinkai");

        // Call the function
        let proxy_mode = fetch_node_proxy_mode();

        // Check the result
        match proxy_mode {
            NodeProxyMode::IsProxied(conf) => {
                assert_eq!(conf.api_peer, "127.0.0.1:9550".parse().unwrap());
                assert_eq!(conf.tcp_peer, "127.0.0.1:9552".parse().unwrap());
                assert_eq!(
                    conf.shinkai_name,
                    ShinkaiName::new("@@node1.shinkai".to_string()).unwrap()
                );
            }
            _ => panic!("Expected IsProxied mode"),
        }
    }

    #[test]
    fn test_fetch_node_no_proxy_mode() {
        // Set the environment variables
        env::set_var("NODE_PROXY_MODE", "SomeOtherMode");

        // Call the function
        let proxy_mode = fetch_node_proxy_mode();

        // Check the result
        match proxy_mode {
            NodeProxyMode::NoProxy => assert!(true),
            _ => panic!("Expected NoProxy mode"),
        }
    }
}
