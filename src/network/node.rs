use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::db::ShinkaiMessageDB;
use crate::shinkai_message::encryption::{
    decrypt_body_content, hash_public_key, string_to_public_key,
};
use crate::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message_proto::ShinkaiMessage;

// Buffer size in bytes.
const BUFFER_SIZE: usize = 2024;

pub enum PingPong {
    Ping,
    Pong,
}

pub enum NodeCommand {
    PingAll,
    GetPublicKey(Sender<PublicKey>),
    SendMessage { msg: ShinkaiMessage },
    ForwardFromProfile { msg: ShinkaiMessage },
    GetPeers(Sender<Vec<SocketAddr>>),
    PkToAddress { pk: String, res: Sender<SocketAddr> },
    Connect { address: SocketAddr, pk: String },
    // add other commands as needed...
}

pub struct Node {
    secret_key: StaticSecret,
    public_key: PublicKey,
    listen_address: SocketAddr,
    peers: CHashMap<(SocketAddr, PublicKey), chrono::DateTime<Utc>>,
    ping_interval_secs: u64,
    commands: Receiver<NodeCommand>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl Node {
    pub fn new(
        listen_address: SocketAddr,
        secret_key: StaticSecret,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        db_path: String,
    ) -> Node {
        let public_key = PublicKey::from(&secret_key);
        let db = ShinkaiMessageDB::new(&db_path)
            .unwrap_or_else(|_| panic!("Failed to open database: {}", db_path));

        Node {
            secret_key,
            public_key,
            peers: CHashMap::new(),
            listen_address,
            ping_interval_secs,
            commands,
            db: Arc::new(Mutex::new(db)),
        }
    }

    pub async fn start(&mut self) -> io::Result<()> {
        let listen_future = self.listen_and_reconnect().fuse();
        pin_mut!(listen_future);

        let ping_interval_secs = if self.ping_interval_secs == 0 {
            315576000 * 100 // 100 years in seconds
        } else {
            self.ping_interval_secs
        };

        let mut ping_interval =
            async_std::stream::interval(Duration::from_secs(ping_interval_secs));
        let mut commands_clone = self.commands.clone();
        // TODO: here we can create a task to check the blockchain for new peers and update our list
        let check_peers_interval_secs = 5;
        let mut check_peers_interval =
            async_std::stream::interval(Duration::from_secs(check_peers_interval_secs));

        loop {
            let ping_future = ping_interval.next().fuse();
            let commands_future = commands_clone.next().fuse();
            let check_peers_future = check_peers_interval.next().fuse();
            pin_mut!(ping_future, commands_future, check_peers_future);

            select! {
                    listen = listen_future => unreachable!(),
                    ping = ping_future => self.ping_all().await?,
                    check_peers = check_peers_future => self.connect_new_peers().await?,
                    command = commands_future => {
                        match command {
                            Some(NodeCommand::PingAll) => self.ping_all().await?,
                            Some(NodeCommand::GetPeers(sender)) => {
                                let peer_addresses: Vec<SocketAddr> = self.peers.clone().into_iter().map(|(k, _)| k.0).collect();
                                sender.send(peer_addresses).await.unwrap();
                            },
                            Some(NodeCommand::PkToAddress { pk, res }) => {
                                let address = Node::pk_to_address(pk);
                                res.send(address).await.unwrap();
                            },
                            Some(NodeCommand::Connect { address, pk }) => {
                                let address_str = address.to_string();
                                let mut db_lock = self.db.lock().await;
                                db_lock.write_to_peers(&pk, &address_str).expect("Failed to write to peers database");
                            },
                            Some(NodeCommand::GetPublicKey(res)) => {
                                let public_key = self.public_key.clone();
                                let _ = res.send(public_key).await.map_err(|_| ());
                            },
                            // Handle other commands
                            _ => break,
                        }
                    }
            };
        }
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
        let mut listener = TcpListener::bind(&self.listen_address).await?;

        println!(
            "{} > TCP: Listening on {}",
            self.listen_address, self.listen_address
        );
        
        loop {
            let (mut socket, addr) = listener.accept().await?;
            let secret_key_clone = self.secret_key.clone();
            let db = Arc::clone(&self.db); 

            tokio::spawn(async move {
                let mut buffer = [0u8; BUFFER_SIZE];
                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) => {
                            // reading 0 bytes signifies the client has closed the connection
                            return;
                        }
                        Ok(n) => {
                            // println!("{} > TCP: Received message.", addr);
                            let _ = Node::handle_message(
                                addr,
                                &buffer[..n],
                                socket.peer_addr().unwrap(),
                                secret_key_clone.clone(),
                                db.clone(),
                            )
                            .await;
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

    async fn connect_new_peers(&self) -> io::Result<()> {
        let db_lock = self.db.lock().await;
        let peer_entries = match db_lock.get_all_peers() {
            Ok(peers) => peers,
            Err(e) => {
                eprintln!("Failed to get peers from database: {}", e);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to get peers from database",
                ));
            }
        };
        drop(db_lock);

        for (pk_str, address_str) in peer_entries {
            let address: SocketAddr = address_str.parse().unwrap();
            let pk = string_to_public_key(&pk_str).unwrap();

            // Check if we already have this peer
            if !self.peers.contains_key(&(address, pk)) {
                // Here we assume there's a function called connect_new_peer that takes a SocketAddr and a public key hash
                // convert SocketAddr to string
                let address_str = address.to_string();
                self.connect(&address_str, pk).await?;
            }
        }
        Ok(())
    }

    // this would require permissions for only allowlist of public keys
    // TODO: add something similar to solidity modifier to check if the sender is allowed
    pub async fn forward_from_profile(&self, message: &ShinkaiMessage) -> io::Result<()> {
        let recipient = message.external_metadata.clone().unwrap().recipient;
        let recipient_pk = string_to_public_key(&recipient).unwrap();

        // for loop over peers
        for peer in self.peers.clone() {
            if peer.0 .1 == recipient_pk {
                let db_lock = self.db.lock().await;
                Node::send(message, peer.0, &db_lock).await?;
                return Ok(());
            }
        }
        println!("Recipient not found in peers");
        Ok(())
    }

    pub fn get_public_key(&self) -> io::Result<PublicKey> {
        Ok(self.public_key)
    }

    pub async fn connect(&self, peer_address: &str, pk: PublicKey) -> io::Result<()> {
        println!(
            "{} > Connecting to {} with pk: {:?}",
            self.listen_address, peer_address, pk
        );
        let peer_address = peer_address.parse().expect("Failed to parse peer ip.");
        self.peers.insert((peer_address, pk), Utc::now());
        Ok(())
    }

    pub async fn send(
        message: &ShinkaiMessage,
        peer: (SocketAddr, PublicKey),
        db: &ShinkaiMessageDB,
    ) -> io::Result<()> {
        // println!("Sending {:?} to {:?}", message, peer);
        let address = peer.0;
        // let mut stream = TcpStream::connect(address).await?;
        let stream = TcpStream::connect(address).await;
        match stream {
            Ok(mut stream) => {
                let encoded_msg = ShinkaiMessageHandler::encode_message(message.clone());
                // println!("send> Encoded Message: {:?}", encoded_msg);
                stream.write_all(encoded_msg.as_ref()).await?;
                stream.flush().await?;
                // info!("Sent message to {}", stream.peer_addr()?);
                db.insert_message(message)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
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
        let mut db_lock = self.db.lock().await;
        for (peer, _) in self.peers.clone() {
            Node::send(message, peer, &db_lock).await?;
        }
        Ok(())
    }

    async fn ping_pong(
        peer: (SocketAddr, PublicKey),
        ping_or_pong: PingPong,
        secret_key: StaticSecret,
        db: &ShinkaiMessageDB,
    ) -> io::Result<()> {
        let message = match ping_or_pong {
            PingPong::Ping => "Ping",
            PingPong::Pong => "Pong",
        };

        let msg = ShinkaiMessageBuilder::ping_pong_message(
            message.to_owned(),
            secret_key.clone(),
            peer.1,
        )
        .unwrap();
        Node::send(&msg, peer, db).await
    }

    pub async fn ping_all(&self) -> io::Result<()> {
        println!(
            "{} > Pinging all peers {} ",
            self.listen_address,
            self.peers.len()
        );
        let mut db_lock = self.db.lock().await;
        for (peer, _) in self.peers.clone() {
            Node::ping_pong(peer, PingPong::Ping, self.secret_key.clone(), &db_lock).await?;
        }
        Ok(())
    }

    async fn send_ack(
        peer: (SocketAddr, PublicKey),
        secret_key: StaticSecret,
        db: &ShinkaiMessageDB,
    ) -> io::Result<()> {
        let ack = ShinkaiMessageBuilder::ack_message(secret_key.clone(), peer.1.clone()).unwrap();
        Node::send(&ack, peer, db).await?;
        Ok(())
    }

    // TODO: this should rely from a database that stores the public keys and their addresses
    // and that db gets updated from the blockchain
    // these are keys created with unsafe_deterministic_private_key starting at 0
    fn pk_to_address(public_key: String) -> SocketAddr {
        match public_key.as_str() {
            "eYy9ZNeMSg+6M4sqY0ljSUDcTltgHbECngLEHg/gVnk=" => {
                SocketAddr::from(([127, 0, 0, 1], 8080))
            }
            "bYByVz2+uwMFd39XMyKGJxaT+MBQhD178V24GkVWRGo=" => {
                SocketAddr::from(([127, 0, 0, 1], 8081))
            }
            "MnPRE+QBohXkKeMnI1IYanNwz37fHi1oqn74eiAjc3E=" => {
                SocketAddr::from(([127, 0, 0, 1], 8082))
            }
            _ => {
                // In real-world scenarios, you'd likely want to return an error here, as an unrecognized
                // public key could lead to problems down the line. The default case should be some sort of
                // error condition.
                println!("Unrecognized public key: {}", public_key);
                SocketAddr::from(([127, 0, 0, 1], 3001))
            }
        }
    }

    async fn handle_message(
        listen_address: SocketAddr,
        bytes: &[u8],
        address: SocketAddr,
        secret_key: StaticSecret,
        maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
    ) -> io::Result<()> {
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
        println!(
            "{} > Sender public key: {:?}",
            listen_address, sender_pk_string
        );

        // if Sender is not part of peers, add it
        // if !self.peers.contains_key(&(address, sender_pk)) {
        //     self.peers.insert((address, sender_pk), Utc::now());
        // }

        let reachable_address = Node::pk_to_address(sender_pk_string.clone());
        match (message_content, message_encryption) {
            ("Ping", _) => {
                println!("{} > Got ping from {:?}", listen_address, address);
                let db = maybe_db.lock().await;
                Node::ping_pong(
                    (reachable_address, sender_pk),
                    PingPong::Pong,
                    secret_key,
                    &db,
                )
                .await?;
            }
            ("ACK", _) => {
                println!("{} > ACK from {:?}", listen_address, address);
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
                        println!(
                            "{} > Got message from {:?}. Sending ACK",
                            listen_address, address
                        );
                        let db = maybe_db.lock().await;
                        Node::send_ack((reachable_address, sender_pk), secret_key, &db).await?;
                    }
                    None => {
                        // TODO: send error back
                        // TODO2: if pk is incorrect, remove from peers
                        println!("Failed to decrypt message.");
                    }
                }
            }
            (_, _) => {
                println!(
                    "{} > Got message from {:?}. Sending ACK",
                    listen_address, address
                );
                let db = maybe_db.lock().await;
                Node::send_ack((reachable_address, sender_pk), secret_key, &db).await?;
            }
        }
        Ok(())
    }
}
