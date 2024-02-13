# VectorFS Documentation

## Introduction

Shinkai's Vector File System (VectorFS) offers a file-system like experience while providing full Vector Search capabilities. The VectorFS does not replace classical operating-system level file systems, but lives within the Shinkai node, acting as both the native storage solution for all content used with AI in the Shinkai node, while also allowing external apps to integrate it into their stack like a next-generation VectorDB.

The VectorFS fully incorporates a global permission system (based on Shinkai identities) thereby securely allowing sharing AI data embeddings with anyone, including with gated whitelists which require delegation/payments (coming Q2 2024).

### Understanding the Basics

At the heart of VectorFS is a hierarchical structure based on paths, where `/` represents the root directory. This structure allows for an intuitive organization of data, mirroring the familiar file system hierarchy found in operating systems.

#### FSEntries: FSFolder and FSItem

Within the VectorFS, every non-root path contains an FSEntry, which can be either an FSFolder or an FSItem. This distinction is crucial for understanding how VectorFS organizes and manages its contents:

- **FSFolders**: These are directory-like structures that can exist at any depth within the VectorFS, starting from the root. An FSFolder can contain other FSFolders (subdirectories) and FSItems (files), allowing for a nested, tree-like organization of data. Folders in VectorFS are not just simple containers; they also hold metadata such as creation, modification, and access times, providing a rich context for the data they contain.

```rust
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
}
```

- **FSItems**: Representing the "files" within VectorFS, FSItems are containers for a single Vector Resource + an optional set of Source Files the Vector Resource was created from (pdfs, docs, txts, etc). FSItems enables the VectorFS to tie original file formats with their vector representations, enhancing the system's utility for a wide range of applications. All FSItems have embeddings, meaning that they are always able to be found via Vector Search in the VectorFS (and be Vector Searched internally as well). Of note, unlike FSFolders, FSItems cannot be placed directly under the root of the file system, but may be placed in any FSFolder.

```rust
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
    /// The original location where the VectorResource/SourceFileMap in this FSItem were downloaded/fetched/synced from.
    pub distribution_origin: DistributionOrigin,
    /// The size in bytes of the Vector Resource in this FSItem
    pub vr_size: usize,
    /// The size in bytes of the SourceFileMap in this FSItem. Will be 0 if no SourceFiles are saved.
    pub source_file_map_size: usize,
    /// Merkle hash, which is in fact the merkle root of the Vector Resource stored in the FSItem
    pub merkle_hash: String,
}
```

### Understanding Permissions in The VectorFS

Every path into the VectorFS which holds an FSEntry has a PathPermission specified for it. The `PathPermission` consists of:

- **ReadPermission**: Determines who can read the contents of a path. It can be one of the following:

  - `Private`: Only the profile owner has read access.
  - `NodeProfiles`: Specific profiles on the same node have read access.
  - `Whitelist`: Only identities explicitly listed in the whitelist have read access.
  - `Public`: Anyone on the Shinkai Network can read the contents.

- **WritePermission**: Determines who can modify the contents of a path.

  - `Private`: Only the profile owner has read access.
  - `NodeProfiles`: Specific profiles on the same node have read access.
  - `Whitelist`: Only identities explicitly listed in the whitelist have read access.

  As of February 2024, the current implementation automatically sets all newly created item/folder permissions to "Whitelist" with no identities added to it. In effect this is equivalent to "Private" as the list is empty, and it simplifies frontend permission management up-front. However do note for advanced use cases, when identities are added to the whitelist it is possible to change the permission to "Private" in order to block access (potentially temporarily) to the path while still preserving the whitelist (whitelists are preserved when changing read/write perms of the path, and only deleted in whole if the FSEntry at the path is fully deleted).

#### Whitelist Permissions

When either the read or write permission for a path is set to Whitelist, then whether a user (Shinkai Name) has access to the path is determined by the whitelist held inside the PathPermission.

In the whitelist each `ShinkaiName` can be set to one of `Read`, `Write`, or `ReadWrite`. This mechanism enables the filesystem to grant or restrict access based on the specific needs of each path.

Folder permissions are also naturally hierarchical as one would expect. This means that if a user is whitelisted for `/folder1/` then they will be automatically whitelisted for an item that is held at `/folder1/my_item`. Do note, `/folder1/my_item` needs to have at least one of its read/write permissions set to `Whitelist` in order for the whitelist on `/folder1/` to apply to my_item.

## Implementation

When a Shinkai Node is initialized, it orchestrates the setup of the Vector File System (VectorFS). The VectorFS is made available as a field within the Node struct as an Arc Mutex to have it be easily accessible across the entire Node.

```rust
pub struct Node {
    ...
    pub vector_fs: Arc<Mutex<VectorFS>>
}
```

### Core Components

The VectorFS comprises several key components, each playing a vital role in the system's functionality:

- **VectorFS**: The central struct that wraps all functionality related to the Vector File System. It maintains a map of `VectorFSInternals` for all profiles on the node, handles the database interactions, and manages permissions and access controls. This is the main struct you will use to interface with the VectorFS.

- **VectorFSInternals**: A struct that contains the internal data of the VectorFS for a specific profile, including permissions, supported embedding models, and everything else.

- **VectorFSDB**: The database layer for the VectorFS, responsible for persisting the file system's state, including profiles, permissions, and file entries.

#### Interacting with VectorFS

To interact with the VectorFS, two extra structs of note are required which deal with all permissioning in a streamlined manner: `VFSReader` and `VFSWriter`.

- **VFSReader**: A struct representing read access rights to the VectorFS under a specific profile and specific path. A `VFSReader` instance is successfully created if permission validation checks pass, allowing for read operations at the path supplied when creating the VFSReader.

- **VFSWriter**: Similar to `VFSReader`, but for write operations. A `VFSWriter` instance grants the ability to perform write actions under a specific profile and specific path, following successful permission validation.

### Workflow

1. **Initialization**: Upon the Shinkai Node's startup, the VectorFS is initialized, setting up the necessary structures for all profiles based on the node's configuration.

2. **Creating Readers and Writers**: Before performing any operations on the VectorFS, a valid `VFSReader` or `VFSWriter` must be created. This involves validating the requester's permissions for the desired action (read or write) at the specified path.

3. **Performing Operations**: With a valid `VFSReader` or `VFSWriter`, various operations can be performed on the VectorFS, such as retrieving file entries, writing data, and managing permissions.

### Example Usage

To read from a specific path in the VectorFS:

1. Create a `VFSReader` by validating read permissions for the requester.
2. Use the `VFSReader` to perform read operations, such as retrieving file entries.

To write to a specific path in the VectorFS:

1. Create a `VFSWriter` by validating write permissions for the requester.
2. Use the `VFSWriter` to perform write operations, such as adding or modifying file entries.

```

```
