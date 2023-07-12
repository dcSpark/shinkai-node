// use rocksdb::{ColumnFamilyDescriptor, Error, Options};

// use std::collections::HashMap;

// use crate::{shinkai_message::shinkai_message_handler::ShinkaiMessageHandler, shinkai_message_proto::ShinkaiMessage};

// use super::{db::Topic, db_errors::ShinkaiMessageDBError, ShinkaiMessageDB};

// #[derive(Debug)]
// pub struct Inbox2 {
//     pub name: String,
//     pub global_perms: HashMap<String, Permission>,
//     pub device_perms: HashMap<String, Permission>,
//     pub agent_perms: HashMap<String, Permission>,
//     pub is_e2e: bool,
//     pub unread_list: Vec<String>,
//     // pub read_list: Vec<String>, // if it's not unread then it's read ;)
// }


// impl Inbox2 {
//     pub fn new(name: &str) -> Inbox2 {
//         Inbox2 {
//             name: name.to_string(),
//             global_perms: HashMap::new(),
//             device_perms: HashMap::new(),
//             agent_perms: HashMap::new(),
//             is_e2e: false,
//             unread_list: vec![],
//         }
//     }

//     // Add permissions for global identities, devices, and agents
//     pub fn add_global_perm(&mut self, identity: &str, perm: Permission) {
//         self.global_perms.insert(identity.to_string(), perm);
//     }

//     pub fn add_device_perm(&mut self, device: &str, perm: Permission) {
//         self.device_perms.insert(device.to_string(), perm);
//     }

//     pub fn add_agent_perm(&mut self, agent: &str, perm: Permission) {
//         self.agent_perms.insert(agent.to_string(), perm);
//     }

//     // Check permissions
//     pub fn has_global_perm(&self, identity: &str, perm: Permission) -> bool {
//         match self.global_perms.get(identity) {
//             Some(p) => *p >= perm,
//             None => false,
//         }
//     }

//     pub fn has_device_perm(&self, device: &str, perm: Permission) -> bool {
//         match self.device_perms.get(device) {
//             Some(p) => *p >= perm,
//             None => false,
//         }
//     }

//     pub fn has_agent_perm(&self, agent: &str, perm: Permission) -> bool {
//         match self.agent_perms.get(agent) {
//             Some(p) => *p >= perm,
//             None => false,
//         }
//     }

//     // Mark message as read/unread
//     pub fn mark_as_read(&mut self, message: &ShinkaiMessage) {
//         // TODO: mark as read should be remove it from unread_list
//         // let hash = ShinkaiMessageHandler::calculate_hash(message);

//         // if let Some(pos) = self.unread_list.iter().position(|id| id == &hash) {
//         //     let id = self.unread_list.remove(pos);
//         //     // self.read_list.push(id);
//         // }
//     }

//     pub fn mark_as_unread(&mut self, message: &ShinkaiMessage) {
//         let hash = ShinkaiMessageHandler::calculate_hash(message);

//         // if let Some(pos) = self.read_list.iter().position(|id| id == &hash) {
//         //     let id = self.read_list.remove(pos);
//         //     self.unread_list.push(id);
//         // }
//     }

//     pub fn name_with_e2e(&self) -> String {
//         format!("{}_{}", self.name, self.is_e2e)
//     }
// }

// pub struct InboxDB {}

// impl InboxDB {
//     pub fn new() -> Self {
//         Self {}
//     }

   
// }
