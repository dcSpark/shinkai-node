// main.rs

mod shinkai_message;
mod network;

mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

use shinkai_node::network::Opt;
use shinkai_node::network::Client;

#[tokio::main]
async fn main() {
    let opt = Opt {
        retries: 5,
        delay: 5,
    };

    let client = Client::new(opt, "127.0.0.1", 8080).await.unwrap();

    // client.send("Ping".to_string());
    // Send more messages...

}
