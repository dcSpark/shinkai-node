use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_subscription_req::FolderSubscription;

use super::fs_entry_tree::FSEntryTree;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SharedFolderInfo {
    pub path: String,
    pub permission: String,
    pub profile: String,
    pub tree: FSEntryTree,
    pub subscription_requirement: Option<FolderSubscription>,
}
