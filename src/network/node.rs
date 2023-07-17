use super::external_identities::ExternalProfileData;
use super::{Identity, IdentityManager};
use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use log::{debug, error, info, trace, warn};
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::db::ShinkaiMessageDB;
use crate::managers::NewIdentityManager;
use crate::network::external_identities::{self, external_identity_to_profile_data};
use crate::network::identities::RegistrationCode;
use crate::network::node_message_handlers::{
    extract_message, extract_recipient_keys, extract_recipient_node_profile_name, extract_sender_node_profile_name,
    get_sender_keys, handle_based_on_message_content_and_encryption, ping_pong, verify_message_signature, PingPong,
};
use crate::shinkai_message::encryption::{
    clone_static_secret_key, decrypt_body_message, encryption_public_key_to_string, encryption_secret_key_to_string,
    string_to_encryption_public_key,
};
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message::signatures::{clone_signature_secret_key, signature_public_key_to_string};
use crate::shinkai_message_proto::ShinkaiMessage;

// Buffer size in bytes.
const BUFFER_SIZE: usize = 2024;

#[derive(Debug)]
pub struct NodeError {
    message: String,
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NodeError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for NodeError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> NodeError {
        NodeError {
            message: format!("{}", err),
        }
    }
}

impl From<std::io::Error> for NodeError {
    fn from(err: std::io::Error) -> NodeError {
        NodeError {
            message: format!("{}", err),
        }
    }
}

pub enum NodeCommand {
    // Command to make the node ping all the other nodes it knows about.
    PingAll,
    // Command to request the node's public keys for signing and encryption. The sender will receive the keys.
    GetPublicKeys(Sender<(SignaturePublicKey, EncryptionPublicKey)>),
    // Command to make the node send a `ShinkaiMessage` in an onionized (i.e., anonymous and encrypted) way.
    SendOnionizedMessage {
        msg: ShinkaiMessage,
    },
    // Command to request the addresses of all nodes this node is aware of. The sender will receive the list of addresses.
    GetPeers(Sender<Vec<SocketAddr>>),
    // Command to make the node create a registration code. The sender will receive the code.
    CreateRegistrationCode {
        res: Sender<String>,
    },
    // Command to make the node use a registration code encapsulated in a `ShinkaiMessage`. The sender will receive the result.
    UseRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<String>,
    },
    // Command to request the external profile data associated with a profile name. The sender will receive the data.
    IdentityNameToExternalProfileData {
        name: String,
        res: Sender<ExternalProfileData>,
    },
    // Command to make the node connect to a new node, given the node's address and profile name.
    Connect {
        address: SocketAddr,
        profile_name: String,
    },
    // Command to fetch the last 'n' messages, where 'n' is defined by `limit`. The sender will receive the messages.
    FetchLastMessages {
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    // Command to request all subidentities that the node manages. The sender will receive the list of subidentities.
    GetAllSubidentities {
        res: Sender<Vec<Identity>>,
    },
    GetLastMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    MarkAsReadUpTo {
        inbox_name: String,
        up_to_time: String,
        res: Sender<String>,
    },
    GetLastUnreadMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    AddInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    RemoveInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    HasInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<bool>,
    },
}

// A type alias for a string that represents a profile name.
type ProfileName = String;

// The `Node` struct represents a single node in the network.
pub struct Node {
    // The profile name of the node.
    pub node_profile_name: String,
    // The secret key used for signing operations.
    pub identity_secret_key: SignatureStaticKey,
    // The public key corresponding to `identity_secret_key`.
    pub identity_public_key: SignaturePublicKey,
    // The secret key used for encryption and decryption.
    pub encryption_secret_key: EncryptionStaticKey,
    // The public key corresponding to `encryption_secret_key`.
    pub encryption_public_key: EncryptionPublicKey,
    // The address this node is listening on.
    pub listen_address: SocketAddr,
    // A map of known peer nodes.
    pub peers: CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>>,
    // The interval at which this node pings all known peers.
    pub ping_interval_secs: u64,
    // The channel from which this node receives commands.
    pub commands: Receiver<NodeCommand>,
    // The manager for subidentities.
    pub subidentity_manager: Arc<Mutex<IdentityManager>>,
    // The database connection for this node.
    pub db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl Node {
    // Construct a new node. Returns a `Result` which is `Ok` if the node was successfully created,
    // and `Err` otherwise.
    pub async fn new(
        node_profile_name: String,
        listen_address: SocketAddr,
        identity_secret_key: SignatureStaticKey,
        encryption_secret_key: EncryptionStaticKey,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        db_path: String,
    ) -> Node {
        // if is_valid_node_identity_name_and_no_subidentities is false panic
        match NewIdentityManager::is_valid_node_identity_name_and_no_subidentities(&node_profile_name.clone()) {
            true => (),
            false => panic!("Invalid node identity name: {}", node_profile_name),
        }

        let identity_public_key = SignaturePublicKey::from(&identity_secret_key);
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let db = ShinkaiMessageDB::new(&db_path).unwrap_or_else(|_| panic!("Failed to open database: {}", db_path));
        let db_arc = Arc::new(Mutex::new(db));
        let subidentity_manager = IdentityManager::new(db_arc.clone()).await.unwrap();

        {
            let db_lock = db_arc.lock().await;
            match db_lock.update_local_node_keys(
                node_profile_name.clone(),
                encryption_public_key.clone(),
                identity_public_key.clone(),
            ) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
            // TODO: maybe check if the keys in the Blockchain match and if not, then prints a warning message to update the keys
        }

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
            subidentity_manager: Arc::new(Mutex::new(subidentity_manager)),
            db: db_arc,
        }
    }

    // Start the node's operations.
    pub async fn start(&mut self) -> Result<(), NodeError> {
        let listen_future = self.listen_and_reconnect().fuse();
        pin_mut!(listen_future);

        let ping_interval_secs = if self.ping_interval_secs == 0 {
            315576000 * 10 // 10 years in seconds
        } else {
            self.ping_interval_secs
        };
        info!("Automatic Ping interval set to {} seconds", ping_interval_secs);

        let mut ping_interval = async_std::stream::interval(Duration::from_secs(ping_interval_secs));
        let mut commands_clone = self.commands.clone();
        // TODO: here we can create a task to check the blockchain for new peers and update our list
        let check_peers_interval_secs = 5;
        let mut check_peers_interval = async_std::stream::interval(Duration::from_secs(check_peers_interval_secs));

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
                            Some(NodeCommand::GetPeers(sender)) => self.send_peer_addresses(sender).await?,
                            Some(NodeCommand::IdentityNameToExternalProfileData { name, res }) => self.handle_external_profile_data(name, res).await?,
                            Some(NodeCommand::Connect { address, profile_name }) => self.connect_node(address, profile_name).await?,
                            Some(NodeCommand::SendOnionizedMessage { msg }) => self.handle_onionized_message(msg).await?,
                            Some(NodeCommand::GetPublicKeys(res)) => self.send_public_keys(res).await?,
                            Some(NodeCommand::FetchLastMessages { limit, res }) => self.fetch_and_send_last_messages(limit, res).await?,
                            Some(NodeCommand::CreateRegistrationCode { res }) => self.create_and_send_registration_code(res).await?,
                            Some(NodeCommand::UseRegistrationCode { msg, res }) => self.handle_registration_code_usage(msg, res).await?,
                            Some(NodeCommand::GetAllSubidentities { res }) => self.get_all_subidentities(res).await?,
                            _ => break,
                        }
                    }
            };
        }
        Ok(())
    }

    // A function that listens for incoming connections and tries to reconnect if a connection is lost.
    async fn listen_and_reconnect(&self) {
        info!("{} > TCP: Starting listen and reconnect loop.", self.listen_address);
        loop {
            match self.listen().await {
                Ok(_) => unreachable!(),
                Err(_) => (),
            }
        }
    }

    // A function that listens for incoming connections.
    async fn listen(&self) -> io::Result<()> {
        let mut listener = TcpListener::bind(&self.listen_address).await?;

        info!("{} > TCP: Listening on {}", self.listen_address, self.listen_address);

        loop {
            let (mut socket, addr) = listener.accept().await?;
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
                            println!("{} > TCP: Received from {:?} : {} bytes.", addr, socket.peer_addr(), n);
                            let destination_socket = socket.peer_addr().expect("Failed to get peer address");
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

    // Get a list of peers this node knows about.
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

    // Connect to a peer node.
    pub async fn connect(&self, peer_address: &str, profile_name: String) -> io::Result<()> {
        info!(
            "{} {} > Connecting to {} with profile_name: {:?}",
            self.node_profile_name, self.listen_address, peer_address, profile_name
        );

        let peer_address = peer_address.parse().expect("Failed to parse peer ip.");
        self.peers.insert((peer_address, profile_name.clone()), Utc::now());

        let peer = (peer_address, profile_name.clone());
        let mut db_lock = self.db.lock().await;

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
            &mut db_lock,
        )
        .await?;
        Ok(())
    }

    // Send a message to a peer.
    pub async fn send(
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        peer: (SocketAddr, ProfileName),
        db: &mut ShinkaiMessageDB,
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
                Node::save_to_db(true, message, my_encryption_sk, db).await?;
                Ok(())
            }
            Err(e) => {
                // handle the error
                println!("Failed to connect to {}: {}", address, e);
                Ok(())
            }
        }
    }

    pub async fn save_to_db(
        am_i_sender: bool,
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        db: &mut ShinkaiMessageDB,
    ) -> io::Result<()> {
        // We want to save it decrypted if possible
        // We are just going to check for the body encryption

        let is_body_encrypted = ShinkaiMessageHandler::is_body_currently_encrypted(message);

        // Clone the message to get a fully owned version
        let mut message_to_save = message.clone();

        // The body should only be decrypted if it's currently encrypted.
        if is_body_encrypted {
            let mut counterpart_identity: String = "".to_string();
            if am_i_sender {
                counterpart_identity = extract_recipient_node_profile_name(message);
            } else {
                counterpart_identity = extract_sender_node_profile_name(message);
            }
            // find the sender's encryption public key in external
            let sender_encryption_pk = external_identity_to_profile_data(counterpart_identity)
                .unwrap()
                .encryption_public_key;

            // Decrypt the message body
            let decrypted_result = decrypt_body_message(&message.clone(), &my_encryption_sk, &sender_encryption_pk);
            match decrypted_result {
                Ok(decrypted_content) => {
                    message_to_save = decrypted_content;
                }
                Err(e) => {
                    println!(
                        "save_to_db> my_encrypt_sk: {:?}",
                        encryption_secret_key_to_string(my_encryption_sk)
                    );
                    println!(
                        "save_to_db> sender_encrypt_pk: {:?}",
                        encryption_public_key_to_string(sender_encryption_pk)
                    );
                    println!("save_to_db> Failed to decrypt message body: {}", e);
                    println!("save_to_db> For message: {:?}", message);
                    return Err(io::Error::new(io::ErrorKind::Other, "Failed to decrypt message body"));
                }
            }
        }

        let db_result = db.insert_inbox_message(&message_to_save);
        match db_result {
            Ok(_) => (),
            Err(e) => {
                println!("Failed to insert message into inbox: {}", e);
                // we will panic for now because that way we can be aware that something is off
                // NOTE: we shouldn't panic on production!
                panic!("Failed to insert message into inbox: {}", e);
            }
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
        info!("{} > Got message from {:?}", receiver_address, unsafe_sender_address);

        // Extract and validate the message
        let message = extract_message(bytes, receiver_address)?;
        println!("{} > Decoded Message: {:?}", receiver_address, message);

        // Extract sender's public keys and verify the signature
        let sender_profile_name_string = extract_sender_node_profile_name(&message);
        let sender_keys = get_sender_keys(&message.clone())?;
        verify_message_signature(sender_keys.signature_public_key, &message)?;

        debug!(
            "{} > Sender Profile Name: {:?}",
            receiver_address, sender_profile_name_string
        );
        debug!("{} > Sender Keys: {:?}", receiver_address, sender_keys);
        debug!("{} > Verified message signature", receiver_address);

        // Save to db
        {
            let mut db = maybe_db.lock().await;
            Node::save_to_db(
                false,
                &message,
                clone_static_secret_key(&my_encryption_secret_key),
                &mut db,
            )
            .await?;
        }

        // println!("who am I: {:?}", my_node_profile_name);

        handle_based_on_message_content_and_encryption(
            message.clone(),
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
