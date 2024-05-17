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
cargo run --release --bin server -- --address <ADDRESS>
```

### Arguments

- `--address`: The address the server will bind to. Default is `0.0.0.0:8080`.

### Example

```sh
cargo run --release --bin server -- --address 127.0.0.1:8080
```

<!-- ## Running the Client

To run the client, use the following command:

```sh
cargo run --release --bin client -- --server-address <ADDRESS> --src-identity <IDENTITY> --dst-identity <IDENTITY>
``` -->
<!-- 
### Arguments

- `--server-address`: The address of the server to connect to. Default is `127.0.0.1:8080`.
- `--src-identity`: The identity of the client. Default is `client1`.
- `--dst-identity`: The identity of the destinationclient. Default is `client2`.

### Example

```sh
cargo run --release --bin client -- --server-address 127.0.0.1:8080 --src-identity client1 --dst-identity client2

cargo run --release --bin client -- --server-address 127.0.0.1:8080 --src-identity client2 --dst-identity client1
```

## Communication Protocol

- Upon connection, the client sends an identity message to the server.
- When sending data, the client specifies the destination identity and the payload.
- The server routes the traffic to the appropriate client based on the destination identity.

### Message Format

Messages are JSON-encoded and prefixed with their length (4 bytes).

#### Identity Message

```json
{
  "Identity": "client1"
}
```

#### Data Message

```json
{
  "Data": {
    "destination": "client2",
    "payload": "Hello, client2!"
  }
}
```

## Example Usage

1. Start the server:

```sh
cargo run --release --bin server -- --address 127.0.0.1:8080
```

2. Start the first client:

```sh
cargo run --release --bin client -- --server-address 127.0.0.1:8080 --src-identity client1
```

3. Start the second client:

```sh
cargo run --release --bin client -- --server-address 127.0.0.1:8080 --src-identity client2 --dst-identity client1
```

4. Clients will send default data and the receiving side will exit. -->
