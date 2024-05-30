# TCP Relayer

This project implements a TCP relayer in Rust that accepts client connections and routes traffic between clients. Clients provide an identity upon connection and specify the destination identity when sending data.

## Prerequisites

- Rust and Cargo installed. You can install Rust using [rustup](https://rustup.rs/).

## Building the Project

To build the project, run the following command in the project directory:

```sh
cargo build --release
```

## Running the Server

To run the server, use the following command:

```sh
cargo run --release --bin server -- --address <ADDRESS> --rpc-url <RPC_URL> --contract-address <CONTRACT_ADDRESS> --identity-secret-key <IDENTITY_SECRET_KEY> --encryption-secret-key <ENCRYPTION_SECRET_KEY> --node-name <NODE_NAME> --open-to-all <OPEN_TO_ALL>
```

### Arguments

- `--address`: The address the server will bind to. Default is `0.0.0.0:8080`.
- `--rpc-url`: RPC URL for the registry.
- `--contract-address`: Contract address for the registry.
- `--identity-secret-key`: Identity secret key (required).
- `--encryption-secret-key`: Encryption secret key (required).
- `--node-name`: Node name (required).
- `--open-to-all`: Open to all clients (true/false). Default is `true`.

### Example

```sh
cargo run --release --bin server -- --address 127.0.0.1:8080 --rpc-url "http://example.com/rpc" --contract-address "0x123..." --identity-secret-key "secret1" --encryption-secret-key "secret2" --node-name "MyNode" --open-to-all true
```
