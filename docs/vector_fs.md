### VectorFS Documentation

#### Introduction

When a Shinkai Node is initialized, it orchestrates the setup of the Vector File System (VectorFS). The VectorFS is made available as a field within the Node struct as an Arc Mutex to have it be easily accessible across the entire Node.

```rust
pub struct Node {
    ...
    pub vector_fs: Arc<Mutex<VectorFS>>
}
```

#### Core Components

The VectorFS comprises several key components, each playing a vital role in the system's functionality:

- **VectorFS**: The central struct that wraps all functionality related to the Vector File System. It maintains a map of `VectorFSInternals` for all profiles on the node, handles the database interactions, and manages permissions and access controls. This is the main struct you will use to interface with the VectorFS.

- **VectorFSInternals**: A struct that contains the internal data of the VectorFS for a specific profile, including permissions, supported embedding models, and everything else.

- **VectorFSDB**: The database layer for the VectorFS, responsible for persisting the file system's state, including profiles, permissions, and file entries.

#### Interacting with VectorFS

To interact with the VectorFS, two extra structs of note are required which deal with all permissioning in a streamlined manner: `VFSReader` and `VFSWriter`.

- **VFSReader**: A struct representing read access rights to the VectorFS under a specific profile and specific path. A `VFSReader` instance is successfully created if permission validation checks pass, allowing for read operations at the path supplied when creating the VFSReader.

- **VFSWriter**: Similar to `VFSReader`, but for write operations. A `VFSWriter` instance grants the ability to perform write actions under a specific profile and specific path, following successful permission validation.

#### Workflow

1. **Initialization**: Upon the Shinkai Node's startup, the VectorFS is initialized, setting up the necessary structures for all profiles based on the node's configuration.

2. **Creating Readers and Writers**: Before performing any operations on the VectorFS, a valid `VFSReader` or `VFSWriter` must be created. This involves validating the requester's permissions for the desired action (read or write) at the specified path.

3. **Performing Operations**: With a valid `VFSReader` or `VFSWriter`, various operations can be performed on the VectorFS, such as retrieving file entries, writing data, and managing permissions.

#### Example Usage

To read from a specific path in the VectorFS:

1. Create a `VFSReader` by validating read permissions for the requester.
2. Use the `VFSReader` to perform read operations, such as retrieving file entries.

To write to a specific path in the VectorFS:

1. Create a `VFSWriter` by validating write permissions for the requester.
2. Use the `VFSWriter` to perform write operations, such as adding or modifying file entries.

```

```
