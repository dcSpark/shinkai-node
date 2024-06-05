use std::net::IpAddr;

use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

pub async fn start_static_server(ip: IpAddr, port: u16, folder_path: String) -> tokio::task::JoinHandle<()> {
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!("Starting static server on {}:{}", ip, port).as_str(),
    );
    let warp_server = warp::serve(warp::fs::dir(folder_path)).run((ip, port));
    tokio::spawn(warp_server)
}