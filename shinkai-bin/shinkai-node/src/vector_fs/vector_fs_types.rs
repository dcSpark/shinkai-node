use super::vector_fs_error::VectorFSError;
use chrono::{DateTime, Utc};
use serde_json::Value;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName, shinkai_utils::job_scope::VectorFSItemScopeEntry,
};
use shinkai_vector_resources::{
    resource_errors::VRError,
    shinkai_time::ShinkaiTime,
    source::DistributionInfo,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, NodeContent, VRHeader, VRKeywords, VRPath},
};
use std::collections::HashMap;

/// Enum that holds the types of external-facing entries used in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FSEntry {
    Folder(FSFolder),
    Item(FSItem),
    Root(FSRoot),
}

impl FSEntry {
    // Attempts to parse the FSEntry into an FSFolder
    pub fn as_folder(self) -> Result<FSFolder, VectorFSError> {
        match self {
            FSEntry::Folder(folder) => Ok(folder),
            FSEntry::Item(i) => Err(VectorFSError::InvalidFSEntryType(i.path.to_string())),
            FSEntry::Root(root) => Err(VectorFSError::InvalidFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the FSEntry into an FSItem
    pub fn as_item(self) -> Result<FSItem, VectorFSError> {
        match self {
            FSEntry::Item(item) => Ok(item),
            FSEntry::Folder(f) => Err(VectorFSError::InvalidFSEntryType(f.path.to_string())),
            FSEntry::Root(root) => Err(VectorFSError::InvalidFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the FSEntry into an FSItem
    pub fn as_root(self) -> Result<FSRoot, VectorFSError> {
        match self {
            FSEntry::Root(root) => Ok(root),
            FSEntry::Item(item) => Err(VectorFSError::InvalidFSEntryType(item.path.to_string())),
            FSEntry::Folder(f) => Err(VectorFSError::InvalidFSEntryType(f.path.to_string())),
        }
    }

    /// Converts the FSEntry to a JSON string
    pub fn to_json(&self) -> Result<String, VectorFSError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Converts the FSEntry to a "simplified" JSON string by calling simplified methods on items/folders/roots
    pub fn to_json_simplified(&self) -> Result<String, VectorFSError> {
        match self {
            FSEntry::Item(item) => item.to_json_simplified(),
            FSEntry::Folder(folder) => folder.to_json_simplified(),
            FSEntry::Root(root) => root.to_json_simplified(),
        }
    }

    /// Converts the FSEntry to a "simplified" JSON Value by calling simplified methods on items/folders/roots
    pub fn to_json_simplified_value(&self) -> Result<Value, VectorFSError> {
        match self {
            FSEntry::Item(item) => item.to_json_simplified_value(),
            FSEntry::Folder(folder) => folder.to_json_simplified_value(),
            FSEntry::Root(root) => root.to_json_simplified_value(),
        }
    }

    /// Converts the FSEntry to a "minimal" JSON Value by calling minimal methods on items/folders/roots
    pub fn to_json_minimal_value(&self) -> Result<Value, VectorFSError> {
        match self {
            FSEntry::Item(item) => item.to_json_minimal_value(),
            FSEntry::Folder(folder) => folder.to_json_minimal_value(),
            FSEntry::Root(root) => root.to_json_minimal_value(),
        }
    }

    /// Creates a FSEntry from a FSEntry JSON string.
    /// Attempts to parse all entry types directly, and then at the top level.
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        if let Ok(folder) = FSFolder::from_json(s) {
            return Ok(FSEntry::Folder(folder));
        }
        if let Ok(item) = FSItem::from_json(s) {
            return Ok(FSEntry::Item(item));
        }
        if let Ok(root) = FSRoot::from_json(s) {
            return Ok(FSEntry::Root(root));
        }

        // If its not any of the specific entries JSON, then fall back on top level parsing
        Ok(serde_json::from_str(s)?)
    }
}

/// An external facing abstraction representing the VecFS root for a given profile.
/// Actual data represented by a FSRoot is the profile's Core MapVectorResource.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSRoot {
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
    // Datetime when the profile's VectorFS was created
    pub created_datetime: DateTime<Utc>,
    /// Datetime which is updated whenever any writes take place. In other words, when
    /// a FSItem or FSFolder is updated/moved/renamed/deleted/etc., last written timestamp is updated.
    pub last_written_datetime: DateTime<Utc>,
    /// Merkle root of the profile's FS
    pub merkle_root: String,
}

impl FSRoot {
    /// Generates a new FSRoot from a MapVectorResource, which is expected to be the FS core resource.
    pub fn from_core_vector_resource(
        resource: MapVectorResource,
        lr_index: &LastReadIndex,
    ) -> Result<Self, VectorFSError> {
        // Generate datetime to suffice the method, this gets ignored in practice when converting back via Self::from
        let current_datetime = ShinkaiTime::generate_time_now();
        let resource = BaseVectorResource::Map(resource);
        let fs_folder = FSFolder::from_vector_resource(resource, VRPath::new(), lr_index, current_datetime)?;
        Ok(Self::from(fs_folder))
    }

    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VectorFSError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }

    /// Converts the FSRoot to a "simplified" JSON string without embeddings in child items
    /// and recursively simplifies child folders.
    pub fn to_json_simplified(&self) -> Result<String, VectorFSError> {
        let mut root = self.clone();
        for child_folder in &mut root.child_folders {
            *child_folder = serde_json::from_str(&child_folder.to_json_simplified()?)?;
        }
        Ok(serde_json::to_string(&root)?)
    }

    /// Converts the FSRoot to a "simplified" JSON Value without embeddings in child items
    /// and recursively simplifies child folders.
    pub fn to_json_simplified_value(&self) -> Result<serde_json::Value, VectorFSError> {
        let mut root = self.clone();
        for child_folder in &mut root.child_folders {
            *child_folder = serde_json::from_value(child_folder.to_json_simplified_value()?)?;
        }
        Ok(serde_json::to_value(&root)?)
    }

    /// Converts the FSRoot to a "minimal" JSON Value without vr_header in child items
    /// and recursively simplifies child folders.
    pub fn to_json_minimal_value(&self) -> Result<serde_json::Value, VectorFSError> {
        let mut root_json = serde_json::to_value(self.clone())?;

        // Recursively simplify child folders to their minimal representation
        if let Some(child_folders) = root_json.get_mut("child_folders").and_then(|cf| cf.as_array_mut()) {
            for folder in child_folders {
                *folder = serde_json::from_value::<FSFolder>(folder.clone())?.to_json_minimal_value()?;
            }
        }

        Ok(root_json)
    }
}

impl From<FSFolder> for FSRoot {
    fn from(folder: FSFolder) -> Self {
        Self {
            path: folder.path,
            child_folders: folder.child_folders,
            created_datetime: folder.created_datetime,
            last_written_datetime: folder.last_written_datetime,
            merkle_root: folder.merkle_hash,
        }
    }
}

/// An external facing folder abstraction used to make interacting with the VectorFS easier.
/// Actual data represented by a FSFolder is a VectorResource-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSFolder {
    /// Name of the FSFolder
    pub name: String,
    /// Path where the FSItem is held in the VectorFS
    pub path: VRPath,
    /// FSFolders which are held within this FSFolder
    pub child_folders: Vec<FSFolder>,
    /// FSItems which are held within this FSFolder
    pub child_items: Vec<FSItem>,
    /// Datetime the FSFolder was first created
    pub created_datetime: DateTime<Utc>,
    /// Datetime the FSFolder was last read by any ShinkaiName
    pub last_read_datetime: DateTime<Utc>,
    /// Datetime the FSFolder was last modified, meaning contents of the directory were changed.
    /// Ie. An FSEntry is moved/renamed/deleted/new one added.
    pub last_modified_datetime: DateTime<Utc>,
    /// Datetime the FSFolder was last written to, meaning any write took place under the folder. In other words, even when
    /// a VR is updated or moved/renamed, then last written is always updated.
    pub last_written_datetime: DateTime<Utc>,
    /// Merkle hash comprised of all of the FSEntries within this folder
    pub merkle_hash: String,
    // pub read_permission:
    // pub write_permission:
}

impl FSFolder {
    /// Initializes a new FSFolder struct
    pub fn new(
        path: VRPath,
        child_folders: Vec<FSFolder>,
        child_items: Vec<FSItem>,
        created_datetime: DateTime<Utc>,
        last_written_datetime: DateTime<Utc>,
        last_read_datetime: DateTime<Utc>,
        last_modified_datetime: DateTime<Utc>,
        merkle_hash: String,
    ) -> Self {
        let name = path.last_path_id().unwrap_or("/".to_string());
        Self {
            name,
            path,
            child_folders,
            child_items,
            created_datetime,
            last_read_datetime,
            last_modified_datetime,
            last_written_datetime,
            merkle_hash,
        }
    }

    /// Initializes a new FSFolder struct with all datetimes set to the current moment.
    pub fn _new_current_time(
        path: VRPath,
        child_folders: Vec<FSFolder>,
        child_items: Vec<FSItem>,
        merkle_hash: String,
    ) -> Self {
        let now = ShinkaiTime::generate_time_now();
        Self::new(
            path,
            child_folders,
            child_items,
            now.clone(),
            now.clone(),
            now.clone(),
            now.clone(),
            merkle_hash,
        )
    }

    /// Generates a new FSFolder using a BaseVectorResource holding Node + the path where the node was retrieved
    /// from in the VecFS internals.
    pub fn from_vector_resource_node(
        node: Node,
        node_fs_path: VRPath,
        lr_index: &LastReadIndex,
    ) -> Result<Self, VectorFSError> {
        // Process datetimes from node
        let last_modified_datetime = Self::process_datetimes_from_node(&node)?;

        match node.content {
            NodeContent::Resource(base_vector_resource) => {
                // Call from_vector_resource with the parsed datetimes
                Self::from_vector_resource(base_vector_resource, node_fs_path, lr_index, last_modified_datetime)
            }
            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
    }

    /// Generates a new FSFolder from a BaseVectorResource + the path where it was retrieved
    /// from inside of the VectorFS.
    fn from_vector_resource(
        resource: BaseVectorResource,
        resource_fs_path: VRPath,
        lr_index: &LastReadIndex,
        last_modified_datetime: DateTime<Utc>,
    ) -> Result<Self, VectorFSError> {
        let mut child_folders = Vec::new();
        let mut child_items = Vec::new();

        // Parse all of the inner nodes
        for node in &resource.as_trait_object().get_root_nodes() {
            match &node.content {
                // If it's a Resource, then create a FSFolder by recursing, and push it to child_folders
                NodeContent::Resource(inner_resource) => {
                    // Process datetimes from node
                    let lm_datetime = Self::process_datetimes_from_node(&node)?;
                    let new_path = resource_fs_path.push_cloned(inner_resource.as_trait_object().name().to_string());
                    child_folders.push(Self::from_vector_resource(
                        inner_resource.clone(),
                        new_path,
                        lr_index,
                        lm_datetime,
                    )?);
                }
                // If it's a VRHeader, then create a FSEntry and push it to child_items
                NodeContent::VRHeader(_) => {
                    let new_path = resource_fs_path.push_cloned(node.id.clone());
                    let fs_item = FSItem::from_vr_header_node(node.clone(), new_path, lr_index)?;
                    child_items.push(fs_item);
                }
                _ => {}
            }
        }

        // Fetch the datetimes/merkle root, and return the created FSFolder
        let last_read_datetime = lr_index.get_last_read_datetime_or_now(&resource_fs_path);
        let created_datetime = resource.as_trait_object().created_datetime();
        let last_written_datetime = resource.as_trait_object().last_written_datetime();
        let merkle_hash = resource.as_trait_object().get_merkle_root()?;
        Ok(Self::new(
            resource_fs_path,
            child_folders,
            child_items,
            created_datetime,
            last_written_datetime,
            last_read_datetime,
            last_modified_datetime,
            merkle_hash,
        ))
    }

    /// Process last_modified datetime in a Node from the VectorFS core resource.
    /// The node must be an FSFolder for this to succeed.
    pub fn process_datetimes_from_node(node: &Node) -> Result<DateTime<Utc>, VectorFSError> {
        // Read last_modified_datetime from metadata
        let last_modified_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::last_modified_key()))
            .ok_or(VectorFSError::InvalidMetadata(Self::last_modified_key()))?;

        // Parse the datetime string
        let last_modified_datetime = ShinkaiTime::from_rfc3339_string(last_modified_str)
            .map_err(|_| VectorFSError::InvalidMetadata(Self::last_modified_key()))?;

        Ok(last_modified_datetime)
    }

    /// Returns the metadata key for the last modified datetime.
    pub fn last_modified_key() -> String {
        String::from("last_modified")
    }

    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VectorFSError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }

    /// Converts the FSFolder to a "simplified" JSON string without embeddings in child items
    /// and recursively simplifies child folders.
    pub fn to_json_simplified(&self) -> Result<String, VectorFSError> {
        let mut folder = self.clone();
        for item in &mut folder.child_items {
            item.vr_header.resource_embedding = None;
            item.vr_header.resource_keywords.keywords_embedding = None;
        }
        for child_folder in &mut folder.child_folders {
            *child_folder = serde_json::from_str(&child_folder.to_json_simplified()?)?;
        }
        Ok(serde_json::to_string(&folder)?)
    }

    /// Converts the FSFolder to a "simplified" JSON Value without embeddings in child items
    /// and recursively simplifies child folders.
    pub fn to_json_simplified_value(&self) -> Result<serde_json::Value, VectorFSError> {
        let mut folder = self.clone();
        for item in &mut folder.child_items {
            item.vr_header.resource_embedding = None;
            item.vr_header.resource_keywords.keywords_embedding = None;
        }
        for child_folder in &mut folder.child_folders {
            *child_folder = serde_json::from_value(child_folder.to_json_simplified_value()?)?;
        }
        Ok(serde_json::to_value(&folder)?)
    }

    /// Converts the FSFolder to a "minimal" JSON Value without vr_header in child items
    /// and recursively simplifies child folders.
    pub fn to_json_minimal_value(&self) -> Result<serde_json::Value, VectorFSError> {
        let mut folder_json = serde_json::to_value(self.clone())?;

        // Simplify child items by removing vr_header
        if let Some(child_items) = folder_json.get_mut("child_items").and_then(|ci| ci.as_array_mut()) {
            for item in child_items {
                item.as_object_mut().unwrap().remove("vr_header");
            }
        }

        // Recursively simplify child folders
        if let Some(child_folders) = folder_json.get_mut("child_folders").and_then(|cf| cf.as_array_mut()) {
            for folder in child_folders {
                *folder = serde_json::from_value::<FSFolder>(folder.clone())?.to_json_minimal_value()?;
            }
        }

        Ok(folder_json)
    }
}

/// An external facing "file" abstraction used to make interacting with the VectorFS easier.
/// Each FSItem always represents a single stored VectorResource, which sometimes also has an optional SourceFileMap.
/// Actual data represented by a FSItem is a VRHeader-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSItem {
    /// Name of the FSItem (based on Vector Resource name)
    pub name: String,
    /// Path where the FSItem is held in the VectorFS
    pub path: VRPath,
    /// The VRHeader matching the Vector Resource stored at this FSItem's path
    pub vr_header: VRHeader,
    /// Datetime the Vector Resource in the FSItem was first created
    pub created_datetime: DateTime<Utc>,
    /// Datetime the Vector Resource in the FSItem was last written to, meaning any updates to its contents.
    pub last_written_datetime: DateTime<Utc>,
    /// Datetime the FSItem was last read by any ShinkaiName
    pub last_read_datetime: DateTime<Utc>,
    /// Datetime the Vector Resource in the FSItem was last saved/updated.
    /// For example when saving a VR into the FS that someone else generated on their node, last_written and last_saved will be different.
    pub vr_last_saved_datetime: DateTime<Utc>,
    /// Datetime the SourceFileMap in the FSItem was last saved/updated. None if no SourceFileMap was ever saved.
    pub source_file_map_last_saved_datetime: Option<DateTime<Utc>>,
    /// The original release location/date time where the VectorResource/SourceFileMap in this FSItem were made available from.
    pub distribution_info: DistributionInfo,
    /// The size of the Vector Resource in this FSItem
    pub vr_size: usize,
    /// The size of the SourceFileMap in this FSItem. Will be 0 if no SourceFiles are saved.
    pub source_file_map_size: usize,
    /// Merkle hash, which is in fact the merkle root of the Vector Resource stored in the FSItem
    pub merkle_hash: String,
}

impl FSItem {
    /// Initialize a new FSItem struct
    pub fn new(
        path: VRPath,
        vr_header: VRHeader,
        created_datetime: DateTime<Utc>,
        last_written_datetime: DateTime<Utc>,
        last_read_datetime: DateTime<Utc>,
        vr_last_saved_datetime: DateTime<Utc>,
        source_file_map_last_saved_datetime: Option<DateTime<Utc>>,
        distribution_info: DistributionInfo,
        vr_size: usize,
        source_file_map_size: usize,
        merkle_hash: String,
    ) -> Self {
        let name = vr_header.resource_name.clone();
        Self {
            name,
            path,
            vr_header,
            created_datetime,
            last_written_datetime,
            last_read_datetime,
            vr_last_saved_datetime,
            source_file_map_last_saved_datetime,
            distribution_info,
            vr_size,
            source_file_map_size,
            merkle_hash,
        }
    }

    /// Returns the name of the FSItem (based on the name in VRHeader)
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// DB key where the Vector Resource matching this FSEntry is held.
    /// Uses the VRHeader reference string. Equivalent to self.resource_reference_string().
    pub fn resource_db_key(&self) -> String {
        self.vr_header.reference_string()
    }

    /// Returns the VRHeader's reference string. Equivalent to self.resource_db_key().
    pub fn resource_reference_string(&self) -> String {
        self.vr_header.reference_string()
    }

    /// Returns the DB key where the SourceFileMap matching this FSEntry is held.
    /// If the FSEntry is marked as having no source file map saved, then returns an VectorFSError.
    pub fn source_file_map_db_key(&self) -> Result<String, VectorFSError> {
        if self.is_source_file_map_saved() {
            Ok(self.resource_db_key())
        } else {
            Err(VectorFSError::NoSourceFileAvailable(self.vr_header.reference_string()))
        }
    }

    /// Checks the last saved datetime to determine if it was ever saved into the FSDB
    pub fn is_source_file_map_saved(&self) -> bool {
        self.source_file_map_last_saved_datetime.is_some()
    }

    /// Generates a new FSItem using a VRHeader holding Node + the path where the node was retrieved
    /// from in the VecFS internals. Use VRPath::new() if the path is root.
    pub fn from_vr_header_node(
        node: Node,
        node_fs_path: VRPath,
        lr_index: &LastReadIndex,
    ) -> Result<Self, VectorFSError> {
        match &node.content {
            NodeContent::VRHeader(header) => {
                // Process data  from node metadata
                let (vr_last_saved_datetime, source_file_map_last_saved) = Self::process_datetimes_from_node(&node)?;
                let last_read_datetime = lr_index.get_last_read_datetime_or_now(&node_fs_path);
                let (vr_size, sfm_size) = Self::process_sizes_from_node(&node)?;
                let merkle_hash = node.get_merkle_hash()?;

                Ok(FSItem::new(
                    node_fs_path,
                    header.clone(),
                    header.resource_created_datetime,
                    header.resource_last_written_datetime,
                    last_read_datetime,
                    vr_last_saved_datetime,
                    source_file_map_last_saved,
                    header.resource_distribution_info.clone(),
                    vr_size,
                    sfm_size,
                    merkle_hash,
                ))
            }

            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
    }

    /// Converts the FSItem into a job scope VectorFSItemScopeEntry
    pub fn as_scope_entry(&self) -> VectorFSItemScopeEntry {
        VectorFSItemScopeEntry {
            name: self.vr_header.resource_name.clone(),
            path: self.path.clone(),
            source: self.vr_header.resource_source.clone(),
        }
    }

    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VectorFSError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }

    /// Converts the FSItem to a "simplified" JSON string without embeddings and keywords
    pub fn to_json_simplified(&self) -> Result<String, VectorFSError> {
        let mut item = self.clone();
        item.vr_header.resource_embedding = None;
        item.vr_header.resource_keywords = VRKeywords::new();
        Ok(serde_json::to_string(&item)?)
    }

    /// Converts the FSItem to a "simplified" JSON Value without embeddings and keywords
    pub fn to_json_simplified_value(&self) -> Result<Value, VectorFSError> {
        let mut item = self.clone();
        item.vr_header.resource_embedding = None;
        item.vr_header.resource_keywords = VRKeywords::new();
        Ok(serde_json::to_value(&item)?)
    }

    /// Converts the FSItem to a "minimal" JSON Value
    pub fn to_json_minimal_value(&self) -> Result<Value, VectorFSError> {
        let mut item_json = serde_json::to_value(self)?;
        // Remove the vr_header from the JSON representation
        item_json.as_object_mut().unwrap().remove("vr_header");
        Ok(item_json)
    }

    /// Process the two last_saved datetimes in a Node from the VectorFS core resource.
    /// The node must be an FSItem for this to succeed.
    pub fn process_datetimes_from_node(node: &Node) -> Result<(DateTime<Utc>, Option<DateTime<Utc>>), VectorFSError> {
        // Read last_saved_datetime from metadata
        let last_saved_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::vr_last_saved_metadata_key()))
            .ok_or(VectorFSError::InvalidMetadata(Self::vr_last_saved_metadata_key()))?;

        // Parse the datetime strings
        let last_saved_datetime = ShinkaiTime::from_rfc3339_string(last_saved_str)
            .map_err(|_| VectorFSError::InvalidMetadata(Self::vr_last_saved_metadata_key()))?;

        // Read source_file_map_saved from metadata, and convert it back into a DateTime<Utc>
        let source_file_map_last_saved = match node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&FSItem::source_file_map_last_saved_metadata_key()))
        {
            Some(s) => Some(ShinkaiTime::from_rfc3339_string(s)?),
            None => None,
        };

        Ok((last_saved_datetime, source_file_map_last_saved))
    }

    /// Process the two sizes stored in metadata in an FSItem Node from the VectorFS core resource.
    /// The node must be an FSItem for this to succeed.
    pub fn process_sizes_from_node(node: &Node) -> Result<(usize, usize), VectorFSError> {
        let vr_size_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::vr_size_metadata_key()));
        let vr_size = match vr_size_str {
            Some(s) => s
                .parse::<usize>()
                .map_err(|_| VectorFSError::InvalidMetadata(Self::vr_size_metadata_key()))?,
            None => 0,
        };

        let sfm_size_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::source_file_map_size_metadata_key()));
        let sfm_size = match sfm_size_str {
            Some(s) => s
                .parse::<usize>()
                .map_err(|_| VectorFSError::InvalidMetadata(Self::source_file_map_size_metadata_key()))?,
            None => 0,
        };

        Ok((vr_size, sfm_size))
    }

    /// Returns the metadata key for the Vector Resource last saved datetime.
    pub fn vr_last_saved_metadata_key() -> String {
        String::from("vr_last_saved")
    }

    /// Metadata key where Vector Resource's size will be found in a Node.
    pub fn vr_size_metadata_key() -> String {
        String::from("vr_size")
    }

    /// Metadata key where Source File Map's last saved datetime will be found in a Node.
    pub fn source_file_map_last_saved_metadata_key() -> String {
        String::from("sfm_last_saved")
    }

    /// Metadata key where SourceFileMap's size will be found in a Node.
    pub fn source_file_map_size_metadata_key() -> String {
        String::from("sfm_size")
    }
}

/// TODO: Implement SubscriptionsIndex later on when it's relevant. For now struct exists
/// to have types roughly in place.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionsIndex {
    pub index: HashMap<VRPath, Vec<ShinkaiName>>,
}

impl SubscriptionsIndex {
    // Creates a new SubscriptionsIndex with the provided index
    pub fn new(index: HashMap<VRPath, Vec<ShinkaiName>>) -> Self {
        Self { index }
    }

    // Creates a new SubscriptionsIndex with an empty index
    pub fn new_empty() -> Self {
        Self { index: HashMap::new() }
    }
}

/// An active in-memory index which holds the last read Datetime of any
/// accessed paths in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LastReadIndex {
    pub index: HashMap<VRPath, (DateTime<Utc>, ShinkaiName)>,
}

impl LastReadIndex {
    // Creates a new LastReadIndex with the provided index
    pub fn new(index: HashMap<VRPath, (DateTime<Utc>, ShinkaiName)>) -> Self {
        Self { index }
    }

    // Creates a new LastReadIndex with an empty index
    pub fn new_empty() -> Self {
        Self { index: HashMap::new() }
    }

    // Updates the last read datetime and name for a given path
    pub fn update_path_last_read(&mut self, path: VRPath, datetime: DateTime<Utc>, name: ShinkaiName) {
        self.index.insert(path, (datetime, name));
    }

    // Retrieves the last read DateTime and ShinkaiName for a given path
    pub fn get_last_read(&self, path: &VRPath) -> Option<&(DateTime<Utc>, ShinkaiName)> {
        self.index.get(path)
    }

    // Retrieves the DateTime when the the FSEntry at the given path was last read
    pub fn get_last_read_datetime(&self, path: &VRPath) -> Option<&DateTime<Utc>> {
        self.index.get(path).map(|tuple| &tuple.0)
    }

    // Retrieves the ShinkaiName who last read the FSEntry at the given path
    pub fn get_last_read_name(&self, path: &VRPath) -> Option<&ShinkaiName> {
        self.index.get(path).map(|tuple| &tuple.1)
    }

    // Retrieves the DateTime when the the FSEntry at the given path was last read, or the current time if not found
    pub fn get_last_read_datetime_or_now(&self, path: &VRPath) -> DateTime<Utc> {
        self.get_last_read_datetime(path)
            .cloned()
            .unwrap_or_else(|| ShinkaiTime::generate_time_now())
    }
}
