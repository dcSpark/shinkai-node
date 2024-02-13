# VectorFS In-Node Rust Documentation

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

Every path into the VectorFS holds an FSEntry has a PathPermission specified for it. The `PathPermission` consists of:

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

- **VectorFSInternals**: A struct that contains the internal data + auxillary metadata of the VectorFS for a specific profile, including permissions, supported embedding models, and everything else.

- **VectorFSDB**: The database layer for the VectorFS, responsible for persisting the file system's state, including profiles, permissions, and file entries.

### Interacting with VectorFS

To interact with the VectorFS, two extra structs of note are required which deal with all permissioning in a streamlined manner: `VFSReader` and `VFSWriter`.

- **VFSReader**: A struct representing read access rights to the VectorFS under a specific profile and specific path. A `VFSReader` instance is successfully created if permission validation checks pass, allowing for read operations at the path supplied when creating the VFSReader.

- **VFSWriter**: Similar to `VFSReader`, but for write operations. A `VFSWriter` instance grants the ability to perform write actions under a specific profile and specific path, following successful permission validation.

Once you have created a Reader you can use the following methods on the VectorFS struct for retrieval from the file system:

- `retrieve_fs_path_simplified_json(&mut self, reader: &VFSReader) -> Result<String, VectorFSError>`: Retrieves a simplified JSON String representation of the FSEntry at the reader's path in the VectorFS. (To sent a summary of a profile's whole FS to frontends, target root `/` with this endpoint)
- `retrieve_vector_resource(reader: &VFSReader) -> Result<BaseVectorResource, VectorFSError>`: Attempts to retrieve a VectorResource from inside an FSItem at the path specified in reader.
- `vector_search_fs_item(reader: &VFSReader, query: Embedding, num_of_results: u64) -> Result<Vec<FSItem>, VectorFSError>`: Performs a vector search into the VectorFS starting at the reader's path, returning the most similar FSItems (which can be converted via `.to_json_simplified()` before passing to frontends).
- And more

Once you have created a Writer, you can use the following methods on the VectorFS struct:

- `create_new_folder(writer: &VFSWriter, new_folder_name: &str) -> Result<FSFolder, VectorFSError>`: Creates a new FSFolder at the writer's path.

- `save_vector_resource_in_folder(writer: &VFSWriter, resource: BaseVectorResource, source_file_map: Option<SourceFileMap>, distribution_origin: DistributionOrigin) -> Result<FSItem, VectorFSError>`: Saves a Vector Resource and optional SourceFileMap into an FSItem, underneath the FSFolder at the writer's path. If an FSItem with the same name (as the VR) already exists underneath the current path, then it updates (overwrites) it. This method does not support saving into the VecFS root.

- `copy_folder(writer: &VFSWriter, destination_path: VRPath) -> Result<FSFolder, VectorFSError>`: Copies the FSFolder from the writer's path into being held underneath the destination_path.

- `copy_item(writer: &VFSWriter, destination_path: VRPath) -> Result<FSItem, VectorFSError>`: Copies the FSItem from the writer's path into being held underneath the destination_path. This method does not support copying into the VecFS root.

- `move_item(writer: &VFSWriter, destination_path: VRPath) -> Result<FSItem, VectorFSError>`: Moves the FSItem from the writer's path into being held underneath the destination_path. This method does not support moving into the VecFS root.

- `move_folder(writer: &VFSWriter, destination_path: VRPath) -> Result<FSFolder, VectorFSError>`: Moves the FSFolder from the writer's path into being held underneath the destination_path. This method supports moving into the VecFS root.

- And more

#### Workflow

1. **Initialization**: Upon the Shinkai Node's startup, the VectorFS is initialized, setting up the necessary structures for all profiles based on the node's configuration.

2. **Creating Readers and Writers**: Before performing any operations on the VectorFS, a valid `VFSReader` or `VFSWriter` must be created. This involves validating the requester_name has permissions for the desired action (read or write) at the specified path (when user is interacting through frontends, then requester_name should be specified to be the user's profile).

3. **Performing Operations**: With a valid `VFSReader` or `VFSWriter`, various operations can be performed on the VectorFS, such as retrieving file entries, writing data, etc.

## Example Usage Inside The Node

#### Creating a Folder

Assuming that we are in the node and have access to the initialized VectorFS (which is set accessible as a field under the `Node` struct ) we can create a folder in root as such:

```rust
    let path = VRPath::new();
    let writer = vector_fs
        .new_writer(requester_name, path.clone(), profile_name)
        .unwrap();
    let folder_name = "first_folder";
    let folder = vector_fs.create_new_folder(&writer, folder_name.clone()).unwrap();
    // And the json can be sent back to the frontend to show all details of the new folder
    let folder_json = folder.to_json_simplified().unwrap();
```

#### Saving A Vector Resource Into An FSItem

Once a folder is created, we can save a BaseVectorResource (ie. One that was generated in the local scope of a job) into as an FSItem in the folder:

```rust
    let folder_path = path.push_cloned(folder_name);
    let writer = vector_fs
        .new_writer(requester_name, folder_path.clone(), profile_name)
        .unwrap();
    let item = vector_fs
        .save_vector_resource_in_folder(
            &writer,
            resource.clone(),
            None,
            DistributionOrigin::None,
        )
        .unwrap();

    // And the json can be sent back to the frontend to show all details of the new item
    let item_json = item.to_json_simplified().unwrap();
```

#### Reading The Whole VectorFS As Json

From there, the whole VectorFS of the profile can be retrieved as a simplified JSON representation for the frontend to visualize via:

```rust
    let reader = vector_fs.new_reader(requester_name, VRPath::root(), profile_name).unwrap();
    let json = vector_fs.retrieve_fs_path_simplified_json(&reader).unwrap();
```

#### Performing Vector Searches On The VectorFS

You can also perform a Vector Search starting from the root (or any path which is a FSFolder) of the VectorFS:

```rust
    let reader = vector_fs.new_reader(requester_name, VRPath::root(), profile_name).unwrap();
    let query_string = "Who is building Shinkai?".to_string();
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();

    // If you just want to return the simplified json representation
    let items = vector_fs.vector_search_fs_item(&reader, query_embedding, 5).unwrap();
    let first_item_json = items[0].to_json_simplified().unwrap();


    // If you want to return the actual VectorResource encoded as JSON (which the encoding stored in .vrkai files)
    let resources = vector_fs.vector_search_vector_resource(&reader, query_embedding, 5).unwrap();
    let resource_json = items[0].to_json().unwrap();
```
