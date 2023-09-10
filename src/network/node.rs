use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use log::{debug, error, info, trace, warn};
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::inbox_name::InboxNameError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_error::ShinkaiMessageError;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, JobToolCall, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::managers::identity_manager::{self};
use crate::managers::job_manager::{JobManager, JobManagerError};
use crate::managers::{job_manager, IdentityManager};
use crate::network::node_message_handlers::{
    extract_message, handle_based_on_message_content_and_encryption, ping_pong, verify_message_signature, PingPong,
};
use crate::schemas::identity::{Identity, StandardIdentity};

use super::node_api::APIError;
use super::node_error::NodeError;

pub enum NodeCommand {
    Shutdown,
    // Command to make the node ping all the other nodes it knows about.
    PingAll,
    // Command to request the node's public keys for signing and encryption. The sender will receive the keys.
    GetPublicKeys(Sender<(SignaturePublicKey, EncryptionPublicKey)>),
    // Command to make the node send a `ShinkaiMessage` in an onionized (i.e., anonymous and encrypted) way.
    SendOnionizedMessage {
        msg: ShinkaiMessage,
        res: async_channel::Sender<Result<(), APIError>>,
    },
    // Command to request the addresses of all nodes this node is aware of. The sender will receive the list of addresses.
    GetPeers(Sender<Vec<SocketAddr>>),
    // Command to make the node create a registration code through the API. The sender will receive the code.
    APICreateRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    // Command to make the node create a registration code locally. The sender will receive the code.
    LocalCreateRegistrationCode {
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        res: Sender<String>,
    },
    // Command to make the node use a registration code encapsulated in a `ShinkaiMessage`. The sender will receive the result.
    APIUseRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    // Command to request the external profile data associated with a profile name. The sender will receive the data.
    IdentityNameToExternalProfileData {
        name: String,
        res: Sender<StandardIdentity>,
    },
    // Command to make the node connect to a new node, given the node's address and profile name.
    Connect {
        address: SocketAddr,
        profile_name: String,
    },
    // Command to make the node connect to a new node, given the node's address and profile name.
    APIConnect {
        address: SocketAddr,
        profile_name: String,
        res: Sender<Result<bool, APIError>>,
    },
    // Command to fetch the last 'n' messages, where 'n' is defined by `limit`. The sender will receive the messages.
    FetchLastMessages {
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    // Command to request all subidentities that the node manages. The sender will receive the list of subidentities.
    APIGetAllSubidentities {
        res: Sender<Result<Vec<StandardIdentity>, APIError>>,
    },
    GetAllSubidentitiesDevicesAndAgents(Sender<Result<Vec<Identity>, APIError>>),
    APIGetAllInboxesForProfile {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIGetLastMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    GetLastMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIMarkAsReadUpTo {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    MarkAsReadUpTo {
        inbox_name: String,
        up_to_time: String,
        res: Sender<String>,
    },
    APIGetLastUnreadMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    GetLastUnreadMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIAddInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    APIRemoveInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
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
    APICreateJob {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    CreateJob {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIJobMessage {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    JobMessage {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIJobPreMessage {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    JobPreMessage {
        tool_calls: Vec<JobToolCall>,
        content: String,
        recipient: String,
        res: Sender<(String, String)>,
    },
    APIAddAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddAgent {
        agent: SerializedAgent,
        res: Sender<String>,
    },
    APIAvailableAgents {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedAgent>, APIError>>,
    },
    AvailableAgents {
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedAgent>, String>>,
    },
}

// A type alias for a string that represents a profile name.
type ProfileName = String;

// The `Node` struct represents a single node in the network.
pub struct Node {
    // The profile name of the node.
    pub node_profile_name: ShinkaiName,
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
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    // The database connection for this node.
    pub db: Arc<Mutex<ShinkaiDB>>,
    // The job manager
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
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
        match ShinkaiName::new(node_profile_name.to_string().clone()) {
            Ok(_) => (),
            Err(_) => panic!("Invalid node identity name: {}", node_profile_name),
        }

        let identity_public_key = SignaturePublicKey::from(&identity_secret_key);
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let db = ShinkaiDB::new(&db_path).unwrap_or_else(|e| {
            eprintln!("Error: {:?}", e);
            panic!("Failed to open database: {}", db_path)
        });
        let db_arc = Arc::new(Mutex::new(db));
        let node_profile_name = ShinkaiName::new(node_profile_name).unwrap();
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

        let subidentity_manager = IdentityManager::new(db_arc.clone(), node_profile_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

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
            identity_manager,
            db: db_arc,
            job_manager: None,
        }
    }

    // Start the node's operations.
    pub async fn start(&mut self) -> Result<(), NodeError> {
        self.job_manager = Some(Arc::new(Mutex::new(
            JobManager::new(
                Arc::clone(&self.db),
                Arc::clone(&self.identity_manager),
                clone_signature_secret_key(&self.identity_secret_key),
                self.node_profile_name.clone(),
            )
            .await,
        )));
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
                            Some(NodeCommand::Shutdown) => {
                                eprintln!("Shutdown command received. Stopping the node.");
                                info!("Shutdown command received. Stopping the node.");
                                break;
                            },
                            Some(NodeCommand::PingAll) => self.ping_all().await?,
                            Some(NodeCommand::GetPeers(sender)) => self.send_peer_addresses(sender).await?,
                            Some(NodeCommand::IdentityNameToExternalProfileData { name, res }) => self.handle_external_profile_data(name, res).await?,
                            Some(NodeCommand::Connect { address, profile_name }) => self.connect_node(address, profile_name).await?,
                            Some(NodeCommand::SendOnionizedMessage { msg, res }) => self.api_handle_send_onionized_message(msg, res).await?,
                            Some(NodeCommand::GetPublicKeys(res)) => self.send_public_keys(res).await?,
                            Some(NodeCommand::FetchLastMessages { limit, res }) => self.fetch_and_send_last_messages(limit, res).await?,
                            Some(NodeCommand::GetAllSubidentitiesDevicesAndAgents(res)) => self.local_get_all_subidentities_devices_and_agents(res).await,
                            Some(NodeCommand::LocalCreateRegistrationCode { permissions, code_type, res }) => self.local_create_and_send_registration_code(permissions, code_type, res).await?,
                            Some(NodeCommand::GetLastMessagesFromInbox { inbox_name, limit, offset_key, res }) => self.local_get_last_messages_from_inbox(inbox_name, limit, offset_key, res).await,
                            Some(NodeCommand::MarkAsReadUpTo { inbox_name, up_to_time, res }) => self.local_mark_as_read_up_to(inbox_name, up_to_time, res).await,
                            Some(NodeCommand::GetLastUnreadMessagesFromInbox { inbox_name, limit, offset, res }) => self.local_get_last_unread_messages_from_inbox(inbox_name, limit, offset, res).await,
                            Some(NodeCommand::AddInboxPermission { inbox_name, perm_type, identity, res }) => self.local_add_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::RemoveInboxPermission { inbox_name, perm_type, identity, res }) => self.local_remove_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::HasInboxPermission { inbox_name, perm_type, identity, res }) => self.has_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::CreateJob { shinkai_message, res }) => self.local_create_new_job(shinkai_message, res).await,
                            Some(NodeCommand::JobMessage { shinkai_message, res }) => self.internal_job_message(shinkai_message).await?,
                            Some(NodeCommand::AddAgent { agent, res }) => self.local_add_agent(agent, res).await,
                            Some(NodeCommand::AvailableAgents { full_profile_name, res }) => self.local_available_agents(full_profile_name, res).await,
                            // Some(NodeCommand::JobPreMessage { tool_calls, content, recipient, res }) => self.job_pre_message(tool_calls, content, recipient, res).await?,
                            // API Endpoints
                            Some(NodeCommand::APICreateRegistrationCode { msg, res }) => self.api_create_and_send_registration_code(msg, res).await?,
                            Some(NodeCommand::APIUseRegistrationCode { msg, res }) => self.api_handle_registration_code_usage(msg, res).await?,
                            Some(NodeCommand::APIGetAllSubidentities { res }) => self.api_get_all_profiles(res).await?,
                            Some(NodeCommand::APIGetLastMessagesFromInbox { msg, res }) => self.api_get_last_messages_from_inbox(msg, res).await?,
                            Some(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res }) => self.api_get_last_unread_messages_from_inbox(msg, res).await?,
                            Some(NodeCommand::APIMarkAsReadUpTo { msg, res }) => self.api_mark_as_read_up_to(msg, res).await?,
                            // Some(NodeCommand::APIAddInboxPermission { msg, res }) => self.api_add_inbox_permission(msg, res).await?,
                            // Some(NodeCommand::APIRemoveInboxPermission { msg, res }) => self.api_remove_inbox_permission(msg, res).await?,
                            Some(NodeCommand::APICreateJob { msg, res }) => self.api_create_new_job(msg, res).await?,
                            Some(NodeCommand::APIGetAllInboxesForProfile { msg, res }) => self.api_get_all_inboxes_for_profile(msg, res).await?,
                            Some(NodeCommand::APIAddAgent { msg, res }) => self.api_add_agent(msg, res).await?,
                            Some(NodeCommand::APIJobMessage { msg, res }) => self.api_job_message(msg, res).await?,
                            Some(NodeCommand::APIAvailableAgents { msg, res }) => self.api_available_agents(msg, res).await?,
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
            let identity_manager = Arc::clone(&self.identity_manager);
            let encryption_secret_key_clone = clone_static_secret_key(&self.encryption_secret_key);
            let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
            let node_profile_name_clone = self.node_profile_name.clone();

            tokio::spawn(async move {
                let mut buffer = Vec::new();
                socket.read_to_end(&mut buffer).await.unwrap();

                let destination_socket = socket.peer_addr().expect("Failed to get peer address");
                let _ = Node::handle_message(
                    addr,
                    destination_socket.clone(),
                    &buffer,
                    node_profile_name_clone.clone().get_node_name(),
                    clone_static_secret_key(&encryption_secret_key_clone),
                    clone_signature_secret_key(&identity_secret_key_clone),
                    db.clone(),
                    identity_manager.clone(),
                )
                .await;
                if let Err(e) = socket.flush().await {
                    eprintln!("Failed to flush the socket: {}", e);
                }
            });
        }
    }

    // indicates if the node is ready or not
    pub async fn is_node_ready(&self) -> bool {
        let identity_manager_guard = self.identity_manager.lock().await;
        identity_manager_guard.is_ready
    }

    // Get a list of peers this node knows about.
    pub fn get_peers(&self) -> CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>> {
        return self.peers.clone();
    }

    // Connect to a peer node.
    pub async fn connect(&self, peer_address: &str, profile_name: String) -> Result<(), NodeError> {
        info!(
            "{} {} > Connecting to {} with profile_name: {:?}",
            self.node_profile_name, self.listen_address, peer_address, profile_name
        );

        let peer_address = peer_address.parse().expect("Failed to parse peer ip.");
        self.peers.insert((peer_address, profile_name.clone()), Utc::now());

        let peer = (peer_address, profile_name.clone());
        let mut db_lock = self.db.lock().await;

        let sender = self.node_profile_name.clone().get_node_name();

        println!(">>> Peer: {:?}", peer);

        let receiver_profile_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&peer.1.clone())
            .await
            .unwrap();
        let receiver = receiver_profile_identity.full_identity_name.get_node_name().to_string();
        let receiver_public_key = receiver_profile_identity.node_encryption_public_key;

        ping_pong(
            peer,
            PingPong::Ping,
            clone_static_secret_key(&self.encryption_secret_key),
            clone_signature_secret_key(&self.identity_secret_key),
            receiver_public_key,
            sender,
            receiver,
            &mut db_lock,
            self.identity_manager.clone(),
        )
        .await?;
        Ok(())
    }

    // Send a message to a peer.
    pub async fn send(
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        peer: (SocketAddr, ProfileName),
        db: &mut ShinkaiDB,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
        save_to_db_flag: bool,
    ) -> Result<(), NodeError> {
        println!("Sending {:?} to {:?}", message, peer);
        let address = peer.0;
        // let mut stream = TcpStream::connect(address).await?;
        let stream = TcpStream::connect(address).await;
        match stream {
            Ok(mut stream) => {
                let encoded_msg = message.encode_message()?;
                // println!("send> Encoded Message: {:?}", encoded_msg);
                stream.write_all(encoded_msg.as_ref()).await?;
                stream.flush().await?;
                info!("Sent message to {}", stream.peer_addr()?);
                if save_to_db_flag {
                    Node::save_to_db(true, message, my_encryption_sk, db, maybe_identity_manager).await?;
                }
                Ok(())
            }
            Err(e) => {
                // handle the error
                println!("Failed to connect to {}: {}", address, e);
                // TODO: it should save the message to db to retry every x^2
                Ok(())
            }
        }
    }

    pub async fn save_to_db(
        am_i_sender: bool,
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        db: &mut ShinkaiDB,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    ) -> io::Result<()> {
        // We want to save it decrypted if possible
        // We are just going to check for the body encryption

        let is_body_encrypted = message.is_body_currently_encrypted();

        // Clone the message to get a fully owned version
        let mut message_to_save = message.clone();

        // The body should only be decrypted if it's currently encrypted.
        if is_body_encrypted {
            let mut counterpart_identity: String = "".to_string();
            // Debug only
            println!("save_to_db> message: {:?}", message.clone());
            if am_i_sender {
                counterpart_identity = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(message)
                    .unwrap()
                    .to_string();
            } else {
                counterpart_identity = ShinkaiName::from_shinkai_message_only_using_sender_node_name(message)
                    .unwrap()
                    .to_string();
            }
            // find the sender's encryption public key in external
            let sender_encryption_pk = maybe_identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&counterpart_identity.clone())
                .await
                .unwrap()
                .node_encryption_public_key;

            // Decrypt the message body
            let decrypted_result = message.decrypt_outer_layer(&my_encryption_sk, &sender_encryption_pk);
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

        // TODO: add identity to this fn so we can check for permissions
        println!("save_to_db> message_to_save: {:?}", message_to_save.clone());
        let db_result = db.unsafe_insert_inbox_message(&message_to_save);
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
        maybe_db: Arc<Mutex<ShinkaiDB>>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    ) -> Result<(), NodeError> {
        info!(
            "\n\n {} > Got message from {:?}",
            receiver_address, unsafe_sender_address
        );

        // Extract and validate the message
        let message = extract_message(bytes, receiver_address)?;
        println!("{} > Decoded Message: {:?}", receiver_address, message);

        // Extract sender's public keys and verify the signature
        let sender_profile_name_string = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&message)
            .unwrap()
            .get_node_name();
        let sender_identity = maybe_identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&sender_profile_name_string)
            .await
            .unwrap();

        verify_message_signature(sender_identity.node_signature_public_key, &message)?;

        debug!(
            "{} > Sender Profile Name: {:?}",
            receiver_address, sender_profile_name_string
        );
        debug!("{} > Node Sender Identity: {}", receiver_address, sender_identity);
        debug!("{} > Verified message signature", receiver_address);

        // Save to db
        {
            let mut db = maybe_db.lock().await;
            Node::save_to_db(
                false,
                &message,
                clone_static_secret_key(&my_encryption_secret_key),
                &mut db,
                maybe_identity_manager.clone(),
            )
            .await?;
        }

        // println!("who am I: {:?}", my_node_profile_name);
        println!("sender identity: {}", sender_identity);

        handle_based_on_message_content_and_encryption(
            message.clone(),
            sender_identity.node_encryption_public_key,
            sender_identity.addr.clone().unwrap(),
            sender_profile_name_string,
            &my_encryption_secret_key,
            &my_signature_secret_key,
            &my_node_profile_name,
            maybe_db,
            maybe_identity_manager,
            receiver_address,
            unsafe_sender_address,
        )
        .await
    }
}
