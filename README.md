<h1 align="center">
  <img src="files/icon.png"/><br/>
  Shinkai Node
</h1>
<p align="center">The Shinkai Node is the central unit within the Shinkai Network that links user devices and oversees AI agents. Its diverse functions include processing user inputs, managing AI models, handling external containerized tooling for AI, coordinating computing tasks, generating proofs, converting and indexing data into vector embeddings, and ensuring efficient task execution according to user needs. The nodes play a crucial role in maintaining the network's decentralized structure, which enhances both security and privacy.<br/><br/> There is a companion repo called Shinkai Apps, that allows you to locally run the node and also easily manage AI models using Ollama, you can find it <a href="https://github.com/dcSpark/shinkai-apps">here</a>.</p><br/>

[![Mutable.ai Auto Wiki](https://img.shields.io/badge/Auto_Wiki-Mutable.ai-blue)](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Documentation

General Documentation: [https://docs.shinkai.com](https://docs.shinkai.com)

More In Depth Codebase Documentation (Mutable.ai): [https://wiki.mutable.ai/dcSpark/shinkai-node](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Installation (Local Compilation)

### Prerequisites

- Rust version >= 1.81 (required for `std::fs::exists` functionality)

Please refer to the installation instructions for your operating system:

- [Windows Installation Instructions](docs/installation/windows.md)
- [Linux Installation Instructions](docs/installation/linux.md)
- [macOS Installation Instructions](docs/installation/macos.md)

## Build

### Easy Build

```
sh scripts/run_node_localhost.sh
```

if you want to restart the node, you can delete the folder `storage` and run the build again. More information at [https://docs.shinkai.com/getting-started](https://docs.shinkai.com/getting-started).

### Build Shinkai Rust Node

```
cargo build
```
Note: You must run this command from the root directory of this repo and make sure that you have set the required ENV variables.

## OpenAPI

### Generate schemas

Run the following command to generate the schema files: 

```
cargo run --example generate_openapi_docs
```

The result will be placed in the folder `docs/openapi`.

### Swagger UI

```
http://{NODE_IP}:{NODE_API_PORT}/v2/swagger-ui/
```

## Tests

Note: You must run these tests from the root directory of this repo.

### Test Shinkai Rust Node Only

Simply use the following to run all rust node tests:

```
cargo test -- --test-threads=1
```

For running a specific test (useful for debugging) you can use:

```
cargo test tcp_node_test -- --nocapture --test-threads=1
```

### Running Dockerized Tests

#### Main tests

```
# Build testing image
docker build -t testing_image -f .github/Dockerfile .

# Run tests main cargo tests
docker run --entrypoint /entrypoints/run-main-cargo-tests.sh testing_image
```

### Further CI Development

Use `act -j test-wasm -P self-hosted=nektos/act-environments-ubuntu:18.04 --container-architecture linux/amd64` to run the tests locally in a docker container. This is useful for debugging CI issues.

## Releasing a New Version

When releasing a new version, ensure that you update the `Cargo.toml` of the shinkai-node as well as the `Cargo.toml` of the shinkai-libs/shinkai-http-api library.
