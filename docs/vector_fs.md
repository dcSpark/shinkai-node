### VectorFS Documentation

#### Introduction

When a Shinkai Node is initialized, it orchestrates the setup of the Vector File System (VectorFS), which is crucial for managing the node's file system in a structured and secure manner. The VectorFS is made available as a field within the node, serving as the backbone for file management and access control within the Shinkai ecosystem.

#### Core Components

The VectorFS comprises several key components, each playing a vital role in the system's functionality:

- **VectorFS**: The central struct that wraps all functionality related to the Vector File System. It maintains a map of `VectorFSInternals` for all profiles on the node, handles the database interactions, and manages permissions and access controls.

- **VectorFSInternals**: A struct that contains the internal details of the VectorFS for a specific profile, including permissions, supported embedding models, and the core resource structure.

- **VectorFSDB**: The database layer for the VectorFS, responsible for persisting the file system's state, including profiles, permissions, and file entries.

#### Interacting with VectorFS

To interact with the VectorFS, two primary interfaces are provided: `VFSReader` and `VFSWriter`. These interfaces are designed to ensure that access to the file system is controlled and that operations are performed securely.

- **VFSReader**: A struct representing read access rights to the VectorFS under a specific profile and path. A `VFSReader` instance is created after passing permission validation checks, allowing for read operations at the specified path.

- **VFSWriter**: Similar to `VFSReader`, but for write operations. A `VFSWriter` instance grants the ability to perform write actions under a specific profile and path, following successful permission validation.

#### Workflow

1. **Initialization**: Upon the Shinkai Node's startup, the VectorFS is initialized, setting up the necessary structures and permissions based on the node's configuration.

2. **Creating Readers and Writers**: Before performing any operations on the VectorFS, a valid `VFSReader` or `VFSWriter` must be created. This involves validating the requester's permissions for the desired action (read or write) at the specified path.

3. **Performing Operations**: With a valid `VFSReader` or `VFSWriter`, various operations can be performed on the VectorFS, such as retrieving file entries, writing data, and managing permissions.

#### Example Usage

To read from a specific path in the VectorFS:

1. Create a `VFSReader` by validating read permissions for the requester.
2. Use the `VFSReader` to perform read operations, such as retrieving file entries.

To write to a specific path in the VectorFS:

1. Create a `VFSWriter` by validating write permissions for the requester.
2. Use the `VFSWriter` to perform write operations, such as adding or modifying file entries.
