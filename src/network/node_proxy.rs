use std::{collections::HashMap, net::SocketAddr};

use serde::{Serialize, Deserialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeProxyMode {
    // Node acts as a proxy, holds identities it proxies for
    // and a flag indicating if it allows new identities
    // if the flag is also then it will also clean up saved identities
    IsProxy(IsProxyConf),
    // Node is being proxied, holds its proxy's identity
    IsProxied(ProxyIdentity),
    // Node is not using a proxy
    NoProxy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IsProxyConf {
    // Flag indicating if new identities can be added
    pub allow_new_identities: bool,
    // Starting node identities
    pub proxy_node_identities: HashMap<String, ProxyIdentity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyIdentity {
    // Address of the API proxy
    pub api_peer: SocketAddr,
    // Address of the TCP proxy
    pub tcp_peer: SocketAddr,
    // Name of the proxied node
    // Or the name of my identity proxied
    pub shinkai_name: ShinkaiName,
}

