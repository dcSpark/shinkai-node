use super::external_identities::ExternalProfileData;
use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::db::{ShinkaiMessageDB};
use crate::network::external_identities::{self, external_identity_to_identity_pk};
use crate::network::node_message_handlers::{
    extract_message, extract_sender_keys, extract_sender_profile_name, verify_message_signature, extract_message_content_and_encryption, handle_based_on_message_content_and_encryption, ping_pong, PingPong,
};
use crate::shinkai_message::encryption::{clone_static_secret_key};
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message::signatures::{clone_signature_secret_key};
use crate::shinkai_message_proto::ShinkaiMessage;

// Buffer size in bytes.
const BUFFER_SIZE: usize = 2024;

pub enum NodeCommand {
    PingAll,
    GetPublicKeys(Sender<(SignaturePublicKey, EncryptionPublicKey)>),
    SendMessage {
        msg: ShinkaiMessage,
    },
    GetPeers(Sender<Vec<SocketAddr>>),
    CreateRegistrationCode {
        res: Sender<String>,
    },
    UseRegistrationCode {
        code: String,
        profile_name: String,
        identity_pk: String,
        encryption_pk: String,
        res: Sender<String>,
    },
    IdentityNameToExternalProfileData {
        name: String,
        res: Sender<ExternalProfileData>,
    },
    Connect {
        address: SocketAddr,
        profile_name: String,
    },
    FetchLastMessages {
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
}

type ProfileName = String;

pub struct Node {
    node_profile_name: String,
    identity_secret_key: SignatureStaticKey,
    identity_public_key: SignaturePublicKey,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    listen_address: SocketAddr,
    peers: CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>>,
    ping_interval_secs: u64,
    commands: Receiver<NodeCommand>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl Node {
    pub fn new(
        node_profile_name: String,
        listen_address: SocketAddr,
        identity_secret_key: SignatureStaticKey,
        encryption_secret_key: EncryptionStaticKey,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        db_path: String,
    ) -> Node {
        let identity_public_key = SignaturePublicKey::from(&identity_secret_key);
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let db = ShinkaiMessageDB::new(&db_path)
            .unwrap_or_else(|_| panic!("Failed to open database: {}", db_path));

        Node {
            node_profile_name,
            identity_secret_key,
            identity_public_key,
            encryption_secret_key,
            encryption_public_key,
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
            315576000 * 10 // 10 years in seconds
        } else {
            self.ping_interval_secs
        };
        println!(
            "Automatic Ping interval set to {} seconds",
            ping_interval_secs
        );

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
            // TODO: update this to read onchain data and update db
            // let check_peers_future = check_peers_interval.next().fuse();
            pin_mut!(ping_future, commands_future);

            select! {
                    listen = listen_future => unreachable!(),
                    ping = ping_future => self.ping_all().await?,
                    // check_peers = check_peers_future => self.connect_new_peers().await?,
                    command = commands_future => {
                        match command {
                            Some(NodeCommand::PingAll) => self.ping_all().await?,
                            Some(NodeCommand::GetPeers(sender)) => {
                                let peer_addresses: Vec<SocketAddr> = self.peers.clone().into_iter().map(|(k, _)| k.0).collect();
                                sender.send(peer_addresses).await.unwrap();
                            },
                            Some(NodeCommand::IdentityNameToExternalProfileData { name, res }) => {
                                let external_profile_data = external_identity_to_identity_pk(name).unwrap();
                                res.send(external_profile_data).await.unwrap();
                            },
                            Some(NodeCommand::Connect { address, profile_name }) => {
                                let address_str = address.to_string();
                                self.connect(&address_str, profile_name).await?;
                            },
                            Some(NodeCommand::SendMessage { msg }) => {
                                // Verify that it's coming from one of our allowed keys
                                let recipient = msg.external_metadata.as_ref().unwrap().recipient.clone();
                                // TODO: fix it
                                // let external_identities_to_identity_pks(&[recipient.clone()]).unwrap();
                                // let address = Node::pk_to_address(recipient.clone());
                                // let pk = string_to_encryption_public_key(&recipient).expect("Failed to convert string to public key");
                                // let db = self.db.lock().await;
                                // Node::send(&msg,(address, pk), &db).await?;
                            },
                            Some(NodeCommand::GetPublicKeys(res)) => {
                                let identity_public_key = self.identity_public_key.clone();
                                let encryption_public_key = self.encryption_public_key.clone();
                                let _ = res.send((identity_public_key, encryption_public_key)).await.map_err(|_| ());
                            },
                            Some(NodeCommand::FetchLastMessages { limit, res }) => {
                                let db = self.db.lock().await;
                                let messages = db.get_last_messages(limit).unwrap_or_else(|_| vec![]);
                                let _ = res.send(messages).await.map_err(|_| ());
                            },
                            Some(NodeCommand::CreateRegistrationCode { res }) => {
                                let db = self.db.lock().await;
                                let code = db.generate_registration_new_code().unwrap_or_else(|_| "".to_string());
                                let _ = res.send(code).await.map_err(|_| ());
                            },
                            Some(NodeCommand::UseRegistrationCode { code, profile_name, identity_pk, encryption_pk, res }) => {
                                let db = self.db.lock().await;
                                let result = db.use_registration_code(&code, &profile_name, &identity_pk, &encryption_pk)
                                    .map_err(|e| e.to_string())
                                    .map(|_| "true".to_string());

                                match result {
                                    Ok(success) => {
                                        let _ = res.send(success).await.map_err(|_| ());
                                    }
                                    Err(e) => {
                                        let _ = res.send(e).await.map_err(|_| ());
                                    }
                                }
                            },
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
            let signature_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
            let db = Arc::clone(&self.db);
            let encryption_secret_key_clone = clone_static_secret_key(&self.encryption_secret_key);
            let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
            let node_profile_name_clone = self.node_profile_name.clone();

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
                            println!(
                                "{} > TCP: Received from {:?} : {} bytes.",
                                addr,
                                socket.peer_addr(),
                                n
                            );
                            let destination_socket =
                                socket.peer_addr().expect("Failed to get peer address");
                            let _ = Node::handle_message(
                                addr,
                                destination_socket.clone(),
                                &buffer[..n],
                                node_profile_name_clone.clone(),
                                clone_static_secret_key(&encryption_secret_key_clone),
                                clone_signature_secret_key(&identity_secret_key_clone),
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

    pub fn get_peers(&self) -> CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>> {
        return self.peers.clone();
    }

    // pub async fn get_encryption_public_key(
    //     &self,
    //     identity_public_key: String,
    // ) -> Result<String, ShinkaiMessageDBError> {
    //     let db = self.db.lock().await;
    //     db.get_encryption_public_key(&identity_public_key)
    // }

    pub async fn connect(&self, peer_address: &str, profile_name: String) -> io::Result<()> {
        println!(
            "{} {} > Connecting to {} with profile_name: {:?}",
            self.node_profile_name, self.listen_address, peer_address, profile_name
        );

        let peer_address = peer_address.parse().expect("Failed to parse peer ip.");
        self.peers
            .insert((peer_address, profile_name.clone()), Utc::now());

        let peer = (peer_address, profile_name.clone());
        let db_lock = self.db.lock().await;

        let sender = self.node_profile_name.clone();
        let receiver_profile = &external_identities::addr_to_external_profile_data(peer.0)[0];
        let receiver = receiver_profile.node_identity_name.to_string();
        let receiver_public_key = receiver_profile.encryption_public_key;

        ping_pong(
            peer,
            PingPong::Ping,
            clone_static_secret_key(&self.encryption_secret_key),
            clone_signature_secret_key(&self.identity_secret_key),
            receiver_public_key,
            sender,
            receiver,
            &db_lock,
        )
        .await?;
        Ok(())
    }

    pub async fn send(
        message: &ShinkaiMessage,
        peer: (SocketAddr, ProfileName),
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

    pub async fn ping_all(&self) -> io::Result<()> {
        println!(
            "{} > Pinging all peers {} ",
            self.listen_address,
            self.peers.len()
        );
        let db_lock = self.db.lock().await;
        for (peer, _) in self.peers.clone() {
            let sender = self.node_profile_name.clone();
            let receiver_profile = &external_identities::addr_to_external_profile_data(peer.0)[0];
            let receiver = receiver_profile.node_identity_name.to_string();
            let receiver_public_key = receiver_profile.encryption_public_key;

            // Important: the receiver doesn't really matter per se as long as it's valid because we are testing the connection
            ping_pong(
                peer,
                PingPong::Ping,
                clone_static_secret_key(&self.encryption_secret_key),
                clone_signature_secret_key(&self.identity_secret_key),
                receiver_public_key,
                sender,
                receiver,
                &db_lock,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn handle_message(
        receiver_address: SocketAddr,
        unsafe_sender_address: SocketAddr,
        bytes: &[u8],
        my_node_profile_name: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
    ) -> io::Result<()> {
        println!(
            "{} > Got message from {:?}",
            receiver_address, unsafe_sender_address
        );

        // Extract and validate the message
        let message = extract_message(bytes, receiver_address)?;
        println!("{} > Decoded Message: {:?}", receiver_address, message);

        // Extract sender's public keys and verify the signature
        let sender_profile_name_string = extract_sender_profile_name(&message);
        let sender_keys = extract_sender_keys(sender_profile_name_string.clone())?;
        verify_message_signature(sender_keys.signature_public_key, &message, receiver_address)?;

        // Save to db
        {
            let db = maybe_db.lock().await;
            db.insert_message(&message.clone())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        // Decode the message and handle it based on its content and encryption
        let (message_content, message_encryption) =
            extract_message_content_and_encryption(&message);
        handle_based_on_message_content_and_encryption(
            &message_content,
            &message_encryption,
            sender_keys.encryption_public_key,
            sender_keys.address,
            sender_profile_name_string,
            &my_encryption_secret_key,
            &my_signature_secret_key,
            &my_node_profile_name,
            maybe_db,
            receiver_address,
            unsafe_sender_address,
        )
        .await
    }
}
