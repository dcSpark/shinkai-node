use super::vector_fs_error::VectorFSError;
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    resource_errors::VRError,
    shinkai_time::ShinkaiTime,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, NodeContent, VRHeader, VRPath},
};
use std::{collections::HashMap, mem::discriminant};

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

    /// Creates a FSEntry from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VectorFSError> {
        Ok(serde_json::from_str(s)?)
    }
}

/// An external facing abstraction representing the VecFS root for a given profile.
/// Actual data represented by a FSRoot is the profile's Core MapVectorResource.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSRoot {
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
    pub child_items: Vec<FSItem>,
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
}

impl From<FSFolder> for FSRoot {
    fn from(folder: FSFolder) -> Self {
        Self {
            path: folder.path,
            child_folders: folder.child_folders,
            child_items: folder.child_items,
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
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
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
        Self {
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
                    let (lm_datetime) = Self::process_datetimes_from_node(&node)?;
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
}

/// An external facing "file" abstraction used to make interacting with the VectorFS easier.
/// Each FSItem always represents a single stored VectorResource, which sometimes also has an optional SourceFileMap.
/// Actual data represented by a FSItem is a VRHeader-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSItem {
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
    /// The original location where the VectorResource/SourceFileMap in this FSItem were downloaded/fetched/synced from.
    pub distribution_origin: DistributionOrigin,
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
        distribution_origin: DistributionOrigin,
        vr_size: usize,
        source_file_map_size: usize,
        merkle_hash: String,
    ) -> Self {
        Self {
            path,
            vr_header,
            created_datetime,
            last_written_datetime,
            last_read_datetime,
            vr_last_saved_datetime,
            source_file_map_last_saved_datetime,
            distribution_origin,
            vr_size,
            source_file_map_size,
            merkle_hash,
        }
    }

    /// Returns the name of the FSItem (based on the name in VRHeader)
    pub fn name(&self) -> String {
        self.vr_header.resource_name.to_string()
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
                let distribution_origin = Self::process_distribution_origin(&node)?;
                let merkle_hash = node.get_merkle_hash()?;

                Ok(FSItem::new(
                    node_fs_path,
                    header.clone(),
                    header.resource_created_datetime,
                    header.resource_last_written_datetime,
                    last_read_datetime,
                    vr_last_saved_datetime,
                    source_file_map_last_saved,
                    distribution_origin,
                    vr_size,
                    sfm_size,
                    merkle_hash,
                ))
            }

            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
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

    /// Process the distribution origin stored in metadata in an FSItem Node from the VectorFS core resource.
    /// The node must be an FSItem for this to succeed.
    pub fn process_distribution_origin(node: &Node) -> Result<DistributionOrigin, VectorFSError> {
        let dist_origin_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::distribution_origin_metadata_key()))
            .ok_or(VectorFSError::InvalidMetadata(Self::distribution_origin_metadata_key()))?;
        Ok(DistributionOrigin::from_json(dist_origin_str)?)
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

    /// Metadata key where DistributionOrigin will be found in a Node.
    pub fn distribution_origin_metadata_key() -> String {
        String::from("dist_origin")
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

/// The origin where a VectorResource was downloaded/acquired from before it arrived
/// in the node's VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DistributionOrigin {
    Uri(String),
    ShinkaiNode((ShinkaiName, VRPath)),
    Other(String),
    None,
}

impl DistributionOrigin {
    // Converts the DistributionOrigin to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    // Creates a DistributionOrigin from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
