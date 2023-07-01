use async_std::net::{TcpListener, TcpStream};
use chashmap::CHashMap;
use chrono::Utc;
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use log::info;
use p2p_node_stats::Stats;
use std::{io, net::SocketAddr, time::Duration};
use x25519_dalek::{PublicKey, StaticSecret};

// Move to another file
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use std::time::SystemTime;

use crate::shinkai_message::encryption::{decrypt_body_content, string_to_public_key};
use crate::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message_proto::ShinkaiMessage;

pub fn gen_random_bytes(n_bytes: usize) -> Vec<u8> {
    let rng = thread_rng();
    rng.sample_iter(Standard).take(n_bytes).collect()
}

pub fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Failed to get duration since UNIX_EPOCH.")
}

pub fn addr_with_port(address: SocketAddr, port: u16) -> SocketAddr {
    let mut address = address;
    address.set_port(port);
    address
}

// Buffer size in bytes.
const BUFFER_SIZE: usize = 2024;
const PING_SIZE: usize = 32;

pub enum PingPong {
    Ping,
    Pong,
}

pub struct Node {
    secret_key: StaticSecret,
    public_key: PublicKey,
    listen_address: SocketAddr,
    peers: CHashMap<(SocketAddr, PublicKey), chrono::DateTime<Utc>>,
    stats: Stats,
}

impl Node {
    pub fn new(
        listen_address: SocketAddr,
        secret_key: StaticSecret,
        public_key: PublicKey,
        stats_window_size: usize,
    ) -> Node {
        Node {
            secret_key,
            public_key,
            peers: CHashMap::new(),
            listen_address,
            stats: Stats::new(stats_window_size, listen_address.to_string()),
        }
    }

    pub async fn start(&self) -> io::Result<()> {
        let listen_future = self.listen_and_reconnect().fuse();
        pin_mut!(listen_future);
        let mut ping_interval = async_std::stream::interval(Duration::from_secs(15));

        loop {
            let ping_future = ping_interval.next().fuse();

            pin_mut!(ping_future);

            select! {
                    listen = listen_future => unreachable!(),
                    ping = ping_future => self.ping_all().await?,
            };
        }
        // self.stats.save_to_file("stats.txt")?;
        Ok(())
    }

    async fn listen_and_reconnect(&self) {
        println!(
            "{} > TCP: Starting listen and reconnect loop.",
            self.listen_address
        );
        loop {
            match self.listen().await {
                Ok(_) => unreachable!(),
                Err(_) => (),
            }
        }
    }
    async fn listen(&self) -> io::Result<()> {
        // let listener = TcpListener::bind(self.listen_address).await?;
        // let mut incoming = listener.incoming(); // trying something new
        // println!(
        //     "{} > TCP: Listening on {}",
        //     self.listen_address, self.listen_address
        // );
        // while let Some(stream) = incoming.next().await {
        //     println!("{} > TCP: Got incoming message.", self.listen_address);
        //     let mut stream = stream?;
        //     let mut buffer = [0u8; BUFFER_SIZE];
        //     let bytes_read = stream.read(&mut buffer).await?;
        //     self.handle_message(&buffer[..bytes_read], stream.peer_addr()?)
        //         .await?;
        //     stream.flush().await?;
        // }
        // Ok(())

        // Method 2
        let mut listener = TcpListener::bind(&self.listen_address).await?;

        println!(
            "{} > TCP: Listening on {}",
            self.listen_address, self.listen_address
        );

        loop {
            let (mut socket, addr) = listener.accept().await?;

            tokio::spawn(async move {
                let mut buffer = [0u8; BUFFER_SIZE];
                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) => {
                            // reading 0 bytes signifies the client has closed the connection
                            println!("{} > TCP: Connection closed.", addr);
                            return;
                        }
                        Ok(n) => {
                            println!("{} > TCP: Received message.", addr);
                            // handle the message
                            // let _ = self
                                // .handle_message(&buffer[..n], socket.peer_addr().unwrap())
                                // .await;
                            // flush the stream
                            if let Err(e) = socket.flush().await {
                                eprintln!("Failed to flush the socket: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("{} > TCP: Failed to read from socket; err = {:?}", addr, e);
                            return;
                        }
                    }
                }
            });
        }
    }

    pub fn get_peers(&self) -> CHashMap<(SocketAddr, PublicKey), chrono::DateTime<Utc>> {
        return self.peers.clone();
    }

    // this would require permissions for only allowlist of public keys
    // TODO: add something similar to solidity modifier to check if the sender is allowed
    pub async fn forward_from_profile(&self, message: &ShinkaiMessage) -> io::Result<()> {
        let recipient = message.external_metadata.clone().unwrap().recipient;
        let recipient_pk = string_to_public_key(&recipient).unwrap();

        // for loop over peers
        for peer in self.peers.clone() {
            if peer.0 .1 == recipient_pk {
                self.send(message, peer.0).await?;
                return Ok(());
            }
        }
        println!("Recipient not found in peers");
        Ok(())
    }

    pub fn get_public_key(&self) -> io::Result<PublicKey> {
        Ok(self.public_key)
    }

    pub async fn start_and_connect(&self, peer_address: &str, pk: PublicKey) -> io::Result<()> {
        println!(
            "{} > Connecting to {} with pk: {:?}",
            self.listen_address, peer_address, pk
        );
        let peer_address = peer_address.parse().expect("Failed to parse peer ip.");
        self.peers.insert((peer_address, pk), Utc::now());
        self.start().await?;
        Ok(())
    }

    pub async fn send(
        &self,
        message: &ShinkaiMessage,
        peer: (SocketAddr, PublicKey),
    ) -> io::Result<()> {
        // println!("Sending {:?} to {:?}", message, peer);
        let address = peer.0;
        // let mut stream = TcpStream::connect(address).await?;
        let stream = TcpStream::connect(address).await;
        match stream {
            Ok(mut stream) => {
                let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(message.clone());
                // println!("send> Encoded Message: {:?}", encoded_msg);
                stream.write_all(encoded_msg.as_ref()).await?;
                stream.flush().await?;
                // info!("Sent message to {}", stream.peer_addr()?);
                Ok(())
            }
            Err(e) => {
                // handle the error
                println!("Failed to connect to {}: {}", address, e);
                Ok(())
            }
        }
    }

    async fn broadcast(&self, message: &ShinkaiMessage) -> io::Result<()> {
        for (peer, _) in self.peers.clone() {
            self.send(message, peer).await?;
        }
        Ok(())
    }

    async fn ping_pong(
        &self,
        peer: (SocketAddr, PublicKey),
        ping_or_pong: PingPong,
    ) -> io::Result<()> {
        let message = match ping_or_pong {
            PingPong::Ping => "Ping",
            PingPong::Pong => "Pong",
        };

        let msg = ShinkaiMessageBuilder::ping_pong_message(
            message.to_owned(),
            self.secret_key.clone(),
            self.public_key.clone(),
        )
        .unwrap();
        self.send(&msg, peer).await
    }

    pub async fn ping_all(&self) -> io::Result<()> {
        println!(
            "{} > Pinging all peers {} ",
            self.listen_address,
            self.peers.len()
        );
        for (peer, _) in self.peers.clone() {
            self.ping_pong(peer, PingPong::Ping).await?;
        }
        Ok(())
    }

    async fn send_ack(&self, peer: (SocketAddr, PublicKey)) -> io::Result<()> {
        let ack =
            ShinkaiMessageBuilder::ack_message(self.secret_key.clone(), peer.1.clone()).unwrap();
        self.send(&ack, peer).await?;
        Ok(())
    }

    async fn handle_message(&self, bytes: &[u8], address: SocketAddr) -> io::Result<()> {
        println!("{} > Got message from {:?}", self.listen_address, address);
        // println!("handle> Encoded Message: {:?}", bytes.to_vec());

        let message = ShinkaiMessageHandler::decode_message(bytes.to_vec());
        let message = match message {
            Ok(message) => message,
            _ => {
                println!("{} > Failed to decode message.", self.listen_address);
                return Ok(());
            }
        };

        let message_content_string = message.body.unwrap().content;
        let message_content = message_content_string.as_str();
        let message_encryption = message.encryption.as_str();
        // println!("Message content: {}", message_content);
        // println!("Encryption: {}", message_encryption);

        let sender_pk_string = message.external_metadata.unwrap().sender;
        let sender_pk = string_to_public_key(sender_pk_string.as_str()).unwrap();

        // if Sender is not part of peers, add it
        if !self.peers.contains_key(&(address, sender_pk)) {
            self.peers.insert((address, sender_pk), Utc::now());
        }

        match (message_content, message_encryption) {
            ("Ping", _) => {
                self.ping_pong((address, sender_pk), PingPong::Pong).await?;
            }
            (_, "default") => {
                let decrypted_content = decrypt_body_content(
                    message_content.as_bytes(),
                    &self.secret_key.clone(),
                    &sender_pk,
                    Some(message_encryption),
                );

                match decrypted_content {
                    Some(_) => {
                        self.send_ack((address, sender_pk)).await?;
                    }
                    None => {
                        // TODO: send error back
                        // TODO2: if pk is incorrect, remove from peers
                        println!("Failed to decrypt message.");
                    }
                }
            }
            (_, _) => {
                self.send_ack((address, sender_pk)).await?;
            }
        }
        Ok(())
    }

    async fn selfless_handle_message(listen_address: SocketAddr, bytes: &[u8], address: SocketAddr, secret_key: StaticSecret) -> io::Result<()> {
        println!("{} > Got message from {:?}", listen_address, address);
        // println!("handle> Encoded Message: {:?}", bytes.to_vec());

        let message = ShinkaiMessageHandler::decode_message(bytes.to_vec());
        let message = match message {
            Ok(message) => message,
            _ => {
                println!("{} > Failed to decode message.", listen_address);
                return Ok(());
            }
        };

        let message_content_string = message.body.unwrap().content;
        let message_content = message_content_string.as_str();
        let message_encryption = message.encryption.as_str();
        // println!("Message content: {}", message_content);
        // println!("Encryption: {}", message_encryption);

        let sender_pk_string = message.external_metadata.unwrap().sender;
        let sender_pk = string_to_public_key(sender_pk_string.as_str()).unwrap();

        // if Sender is not part of peers, add it
        if !self.peers.contains_key(&(address, sender_pk)) {
            self.peers.insert((address, sender_pk), Utc::now());
        }

        match (message_content, message_encryption) {
            ("Ping", _) => {
                // self.ping_pong((address, sender_pk), PingPong::Pong).await?;
                println!("{} > Got ping from {:?}", listen_address, address);
            }
            (_, "default") => {
                let decrypted_content = decrypt_body_content(
                    message_content.as_bytes(),
                    &secret_key.clone(),
                    &sender_pk,
                    Some(message_encryption),
                );

                match decrypted_content {
                    Some(_) => {
                        // self.send_ack((address, sender_pk)).await?;
                        println!("{} > Got message from {:?}", listen_address, address);
                    }
                    None => {
                        // TODO: send error back
                        // TODO2: if pk is incorrect, remove from peers
                        println!("Failed to decrypt message.");
                    }
                }
            }
            (_, _) => {
                self.send_ack((address, sender_pk)).await?;
            }
        }
        Ok(())
    }
}
