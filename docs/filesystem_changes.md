# Filesystem Changes in Shinkai Node

## Overview
This document describes the transition from virtual inboxes to a real filesystem implementation in Shinkai Node, including changes to the `JobMessage` structure and the introduction of the `ShinkaiPath` system.

## Major Changes

### Removal of Inboxes
Previously, inboxes were implemented as virtual constructs that simulated folders while storing blobs in a database. The system has been updated to use actual folders and files in the filesystem, providing a 1:1 mapping between virtual and physical files.

### Updated JobMessage Structure
The `JobMessage` struct has been updated to reflect these changes:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct JobMessage {
    pub job_id: String,
    pub content: String,
    pub parent: Option<String>,
    pub sheet_job_data: Option<String>,
    // Whenever we need to chain actions, we can use this
    pub callback: Option<Box<CallbackAction>>,
    // This is added from the node
    pub metadata: Option<MessageMetadata>,
    // Whenever we want to force the use of a specific tool, we can use this
    pub tool_key: Option<String>,
    // Field that lists associated files of the message
    #[serde(default)]
    pub fs_files_paths: Vec<ShinkaiPath>,
    #[serde(default)]
    pub job_filenames: Vec<String>,
}
```

## File Path Handling

### fs_files_paths
The `fs_files_paths` field uses `ShinkaiPath` to represent relative paths that appear as absolute paths from the node's perspective. 

Example:
```
Folder: legal
File: buying my dog a house contract.pdf
fs_files_paths entry: legal/buying my dog a house contract.pdf
```

### job_filenames
The `job_filenames` field is used for files uploaded directly to a job. Job folders are created with a specific naming convention:
```
Format: {Month} {Day} - ({JobID last 4 chars}) {First message...}
Example: Dec 26 - (89AF) Tell me what this...
```

When uploading a file to a job (e.g., `my cat.jpg`), you only need to specify the filename in `job_filenames` rather than the full path.

### ShinkaiPath
The `ShinkaiPath` struct handles path conversions at the node level:
- Provides a consistent way to handle file paths across the system
- Offers `.full_path()` method to get the absolute path when needed
- Includes comprehensive test coverage for various scenarios

## Docker Mount Considerations

### Current Limitations
- Cannot mount two files with the same name from different folders
- Example: If you have `folder1/config.json` and `folder2/config.json`, you can't mount both simultaneously

### Workaround
A file map implementation in the code runner side is being developed to address this limitation. From the code runner's perspective, only the real (absolute) path is required for operation.

## Implementation Notes
- `ShinkaiFileManager` and `ShinkaiPath` include comprehensive tests for various scenarios
- Path conversion happens automatically at the node level
- The system maintains backward compatibility while providing a more robust file handling mechanism
