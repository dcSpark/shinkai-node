use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::Opt;
use crate::shinkai_message::{
    shinkai_message_builder::ShinkaiMessageBuilder, shinkai_message_handler::ShinkaiMessageHandler,
};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use x25519_dalek::{StaticSecret, PublicKey};

pub struct Client {
    writer: mpsc::UnboundedSender<Vec<u8>>,
    reader: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    reading: Arc<AtomicBool>,
}

impl Client {
    pub async fn new(opt: Opt, ip: &str, port: u16) -> tokio::io::Result<Self> {
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (read_tx, read_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let addr = format!("{}:{}", ip, port);
        let stream = TcpStream::connect(addr).await?;
        let (mut reader, mut writer) = split(stream);
        let reading = Arc::new(AtomicBool::new(true));
        
        let reading_clone = Arc::clone(&reading);
        tokio::spawn(async move {
            loop {
                if let Some(msg) = write_rx.recv().await {
                    if reading_clone.load(Ordering::Relaxed) { // only try to write if the reading task is still running
                        if let Err(e) = writer.write_all(&msg).await {
                            println!("Failed to write to socket, error: {}", e);
                        }
                    }
                }
            }
        });

        let reading_clone = Arc::clone(&reading);
        tokio::spawn(async move {
            let mut buffer = vec![0; 1024];
            loop {
                if let Ok(n) = reader.read(&mut buffer).await {
                    let received_message = buffer[..n].to_vec();
                    if let Ok(shinkai_message) = ShinkaiMessageHandler::decode_message(received_message.clone()) {
                        if let Some(body) = shinkai_message.body {
                            println!("Received message from server: {}", body.content); // Add this line
                            if body.content == "terminate" {
                                println!("Termination signal received from the server. Stopping reading task.");
                                reading_clone.store(false, Ordering::Relaxed); // set the flag to false when stopping the reading task
                                break; // stop the loop
                            }
                        }
                    }
                    if let Err(e) = read_tx.send(received_message) {
                        println!("Failed to send read message, error: {}", e);
                    }
                }
            }
        });

        Ok(Client {
            writer: write_tx,
            reader: Some(read_rx), // put the receiver into the optional
            reading,
        })
    }

    pub async fn send(&mut self, msg: Vec<u8>) -> Option<Vec<u8>> {
        self.writer.send(msg).unwrap();
        if let Some(reader) = &mut self.reader {
            match reader.recv().await {
                Some(msg) => Some(msg),
                None => {
                    println!("Failed to receive message: channel closed");
                    None
                }
            }
        } else {
            println!("Failed to receive message: reader is None");
            None
        }
    }
    
    pub async fn terminate(&mut self, secret_key: StaticSecret, public_key: PublicKey) {
        let terminate_message_result =
            ShinkaiMessageBuilder::terminate_message(secret_key, public_key);
        let terminate_msg = match terminate_message_result {
            Ok(msg) => msg,
            Err(e) => panic!("Failed to create a termination message: {:?}", e),
        };
        let encoded_terminate_msg = ShinkaiMessageHandler::encode_shinkai_message(terminate_msg);
        let _ = self.send(encoded_terminate_msg).await;

        while self.reading.load(Ordering::Relaxed) {
            // wait for the reading task to finish
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        self.reader.take(); // take the receiver out, thus dropping it
    }
}
