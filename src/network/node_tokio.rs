// use std::sync::atomic::{AtomicBool, Ordering, AtomicUsize};
// use std::sync::Arc;
// use std::time::Duration;

// use super::Opt;
// use crate::shinkai_message::encryption::{decrypt_body_content, string_to_public_key};
// use crate::shinkai_message::{
//     shinkai_message_builder::ShinkaiMessageBuilder, shinkai_message_handler::ShinkaiMessageHandler,
// };
// use tokio::{
//     io::{split, AsyncReadExt},
//     net::{TcpListener, TcpStream},
//     sync::mpsc,
// };
// use x25519_dalek::{PublicKey, StaticSecret};

// pub struct Node {
//     writer: mpsc::UnboundedSender<Vec<u8>>,
//     reader: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
//     reading: Arc<AtomicBool>,
//     secret_key: StaticSecret,
//     public_key: PublicKey,
//     connections: Arc<AtomicUsize>,
// }

// impl Node {
//     pub async fn new(
//         opt: Opt,
//         secret_key: StaticSecret,
//         public_key: PublicKey,
//         listener_addr: String,
//     ) -> tokio::io::Result<Self> {
//         let secret_key_clone = secret_key.clone();
//         let public_key_clone = public_key.clone();
//         let (write_tx, mut _write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
//         let write_tx_clone = write_tx.clone();
//         let (read_tx, read_rx) = mpsc::unbounded_channel::<Vec<u8>>();

//         let listener = TcpListener::bind(&listener_addr).await?;
//         let local_addr = listener.local_addr()?;

//         let reading = Arc::new(AtomicBool::new(true));
//         let reading_clone = Arc::clone(&reading);

//         let connections = Arc::new(AtomicUsize::new(0));
//         let connections_clone = Arc::clone(&connections);

//         tokio::spawn(async move {
//             print!("Inside the tokio::spawn(async move) block");
//             loop {
//                 if !reading_clone.load(Ordering::Relaxed) {
//                     break;
//                 }
//                 print!("Listening on: {} ", local_addr);
//                 match listener.accept().await {
//                     Ok((stream, _)) => {
//                         connections_clone.fetch_add(1, Ordering::SeqCst);
//                         let (mut reader, _writer) = split(stream);
//                         let mut buffer = vec![0; 4048];
//                         // print some debugging code
//                         print!("after buffer");

//                         loop {
//                             if let Ok(n) = reader.read(&mut buffer).await {
//                                 let received_message = buffer[..n].to_vec();
//                                 // print the received_message
//                                 println!("Received message: {:?}", received_message);

//                                 if let Ok(shinkai_message) =
//                                     ShinkaiMessageHandler::decode_message(received_message.clone())
//                                 {
//                                     let message_content_string =
//                                         shinkai_message.body.unwrap().content;
//                                     let message_content = message_content_string.as_str();
//                                     let message_encryption = shinkai_message.encryption.as_str();

//                                     // print the message_content_string
//                                     println!("Received message: {}", message_content_string);
//                                     match (message_content, message_encryption) {
//                                         ("Ping", _) => {
//                                             let pong = ShinkaiMessageBuilder::ping_pong_message(
//                                                 "Pong".to_owned(),
//                                                 secret_key_clone.clone(),
//                                                 public_key_clone.clone(),
//                                             )
//                                             .unwrap();
//                                             let encoded_msg =
//                                                 ShinkaiMessageHandler::encode_shinkai_message(pong);
//                                             if let Err(e) = write_tx_clone.send(encoded_msg) {
//                                                 println!(
//                                                     "Failed to send Pong message, error: {}",
//                                                     e
//                                                 );
//                                             }
//                                         }
//                                         ("terminate", _) => {
//                                             println!("Termination signal received from the peer. Stopping reading task.");
//                                             reading_clone.store(false, Ordering::Relaxed);
//                                             break;
//                                         }
//                                         (_, "default") => {
//                                             let sender_pk_string =
//                                                 shinkai_message.external_metadata.unwrap().sender;
//                                             let sender_pk =
//                                                 string_to_public_key(sender_pk_string.as_str())
//                                                     .unwrap();
//                                             let decrypted_content = decrypt_body_content(
//                                                 message_content.as_bytes(),
//                                                 &secret_key_clone.clone(),
//                                                 &sender_pk,
//                                                 Some(message_encryption),
//                                             );

//                                             match decrypted_content {
//                                                 Some(_) => {
//                                                     let ack = ShinkaiMessageBuilder::ack_message(
//                                                         secret_key_clone.clone(),
//                                                         public_key_clone.clone(),
//                                                     )
//                                                     .unwrap();
//                                                     let encoded_msg =
//                                                         ShinkaiMessageHandler::encode_shinkai_message(ack);
//                                                     let _ = write_tx_clone
//                                                         .send(encoded_msg)
//                                                         .expect("Failed to send ACK message");
//                                                 }
//                                                 None => {
//                                                     println!("Failed to decrypt message.");
//                                                     continue;
//                                                 }
//                                             }
//                                         }
//                                         (_, _) => {
//                                             let ack = ShinkaiMessageBuilder::ack_message(
//                                                 secret_key_clone.clone(),
//                                                 public_key_clone.clone(),
//                                             )
//                                             .unwrap();
//                                             let encoded_msg =
//                                                 ShinkaiMessageHandler::encode_shinkai_message(ack);
//                                             let _ = write_tx_clone
//                                                 .send(encoded_msg)
//                                                 .expect("Failed to send ACK message");
//                                         }
//                                     }
//                                 }
//                                 if let Err(e) = read_tx.send(received_message) {
//                                     println!("Failed to send read message, error: {}", e);
//                                     connections_clone.fetch_sub(1, Ordering::SeqCst);
//                                     break;
//                                 }
//                             }
//                         }
//                     }
//                     Err(e) => {
//                         println!("Failed to accept connection, error: {}", e);
//                         continue;
//                     }
//                 }
//             }
//         });

//         // let result = handle.await;

//         Ok(Node {
//             // listener,
//             writer: write_tx,
//             reader: Some(read_rx),
//             reading,
//             secret_key,
//             public_key,
//             connections
//         })
//     }

//     pub async fn connect_to_peer(&self, ip: &str, port: u16) -> tokio::io::Result<()> {
//         let addr = format!("{}:{}", ip, port);
//         let stream = TcpStream::connect(addr).await?;
//         let (mut reader, _writer) = split(stream);
    
//         let secret_key_clone = self.secret_key.clone();
//         let public_key_clone = self.public_key.clone();
//         let reading_clone = Arc::clone(&self.reading);
//         let write_tx_clone = self.writer.clone();
//         let connections_clone = Arc::clone(&self.connections);
    
//         connections_clone.fetch_add(1, Ordering::SeqCst);
    
//         // Start a new task for handling the connection
//         tokio::spawn(async move {
//             let mut buffer = vec![0; 4048];
//             loop {
//                 if !reading_clone.load(Ordering::Relaxed) {
//                     break;
//                 }
//                 if let Ok(n) = reader.read(&mut buffer).await {
//                     // your logic here
//                     // print the received_message
//                     let received_message = buffer[..n].to_vec();
//                     println!("Received message: {:?}", received_message);
                    
//                 }
//             }
//             connections_clone.fetch_sub(1, Ordering::SeqCst);
//         });
    
//         Ok(())
//     }
    

//     pub async fn send(
//         &mut self,
//         msg: Vec<u8>,
//     ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
//         self.writer.send(msg)?;
//         if let Some(reader) = &mut self.reader {
//             match reader.recv().await {
//                 Some(msg) => Ok(Some(msg)),
//                 None => {
//                     println!("Failed to receive message: channel closed");
//                     Ok(None)
//                 }
//             }
//         } else {
//             println!("Failed to receive message: reader is None");
//             Ok(None)
//         }
//     }

//     pub async fn terminate(&mut self) {
//         let terminate_message_result =
//             ShinkaiMessageBuilder::terminate_message(self.secret_key.clone(), self.public_key);
//         let terminate_msg = match terminate_message_result {
//             Ok(msg) => msg,
//             Err(e) => panic!("Failed to create a termination message: {:?}", e),
//         };
//         let encoded_terminate_msg = ShinkaiMessageHandler::encode_shinkai_message(terminate_msg);
//         let _ = self.send(encoded_terminate_msg).await;

//         while self.reading.load(Ordering::Relaxed) {
//             // wait for the reading task to finish
//             tokio::time::sleep(Duration::from_millis(100)).await;
//         }
//         self.reader.take(); // take the receiver out, thus dropping it
//     }

//     pub async fn receive(&mut self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
//         if let Some(reader) = &mut self.reader {
//             match reader.recv().await {
//                 Some(msg) => Ok(Some(msg)),
//                 None => {
//                     println!("Failed to receive message: channel closed");
//                     Ok(None)
//                 }
//             }
//         } else {
//             println!("Failed to receive message: reader is None");
//             Ok(None)
//         }
//     }

//     pub fn is_up_and_running(&self) -> bool {
//         self.reading.load(Ordering::Relaxed)
//     }

//     pub fn connection_count(&self) -> usize {
//         self.connections.load(Ordering::Relaxed)
//     }
// }