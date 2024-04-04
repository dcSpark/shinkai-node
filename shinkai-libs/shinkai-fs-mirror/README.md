# Shinkai FS Mirror

Shinkai FS Mirror is a tool designed to synchronize filesystem changes with Shinkai, ensuring that your data is always up-to-date across different environments.

## Prerequisites

Before you compile and run Shinkai FS Mirror, ensure you have the following installed on your system:

- Rust (latest stable version recommended)
- Cargo (Rust's package manager, comes with Rust)
- Optionally, `dotenv` for managing environment variables locally

## Compilation

To compile the project, follow these steps:

1. Open a terminal.
2. Navigate to the root directory of the project (`shinkai_fs_mirror`).
3. Run the following command to compile the project:

```bash
cargo build --release
```

This command compiles the project in release mode, optimizing the binary for performance. The compiled binary will be located in `target/release/`.

## Running the Project

To run Shinkai FS Mirror, you can use Cargo or execute the binary directly. Here are the steps for both methods:

### Using Cargo

In the project's root directory, run:

```bash
cargo run --release -- -f <encrypted_file_path> -p <passphrase> -d <destination_path> -w <folder_to_watch> -b <db_path> -i <sync_interval>
```

Example:
```bash
cargo run -- --file "local_shinkai.key" --pass "mypass" --dest "/youtube" --watch "./youtube" --db "mirror_db" --interval "immediate"
```

Replace `<encrypted_file_path>`, `<passphrase>`, `<destination_path>`, `<folder_to_watch>`, `<db_path>`, and `<sync_interval>` with your actual values.

## Configuration

Shinkai FS Mirror can be configured using command-line arguments or environment variables. The following are the available options:

- `-f` or `--file`: Sets the path to the encrypted file containing keys.
- `-p` or `--pass`: Passphrase for the encrypted file. Can also be set via the `PASSPHRASE` environment variable.
- `-d` or `--dest`: **Required.** Destination path for the synchronization.
- `-w` or `--watch`: **Required.** Folder path to watch for changes.
- `-b` or `--db`: Database path for storing synchronization data. Defaults to `mirror_db` if not specified.
- `-i` or `--interval`: Sync interval (immediate, timed:<seconds>, none). Defaults to `immediate` if not specified.

Ensure you have the `.env` file in the root directory if you prefer to use environment variables for configuration.

## Support

For any issues or questions, please refer to the project's GitHub issues page or contact the author at `nico@shinkai.com`.