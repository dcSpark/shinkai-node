<h1 align="center">
  <img src="files/icon.png"/><br/>
  Shinkai Node
</h1>
<p align="center">Shinkai allows you to create AI agents without touching code. Define tasks, schedule actions, and let Shinkai write custom code for you. Native crypto support included.<br/><br/> There is a companion repo called Shinkai Apps which contains the frontend that encapsulates this project, you can find it <a href="https://github.com/dcSpark/shinkai-apps">here</a>.</p><br/>

## Overview

Shinkai Node is the backend service that powers the Shinkai network of AI agents. It exposes an HTTP API, manages jobs and tasks, runs inference chains and handles vector searches. The project is written in Rust and organized as a cargo workspace composed of several crates.

### Repository Structure

- `shinkai-bin/shinkai-node/` – main executable crate. The entry point is `src/main.rs` which starts the node via `runner.rs`.
- `shinkai-libs/` – collection of library crates used by the node (crypto identities, message primitives, filesystem, embedding, HTTP API, tools primitives, sqlite helpers, etc.).
- `docs/` – architecture notes, installation guides, OpenAPI schemas and testing documentation.
- `scripts/` – helper scripts for local development such as `run_node_localhost.sh`.
- `cloud-node/` – example configuration for running the node on cloud VMs.
- `perf_scripts/` – scripts for performance testing.

You can browse the documentation in the `docs/` folder to learn more about inference chains, endpoint creation and testing practices.

## Documentation

General Documentation: [https://docs.shinkai.com](https://docs.shinkai.com)

## Installation (Local Compilation)

### Prerequisites

- Rust version >= 1.85 (required for `std::fs::exists` functionality)

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
IS_TESTING=1 cargo test -- --test-threads=1
```

For running a specific test (useful for debugging) you can use:

```
IS_TESTING=1 cargo test tcp_node_test -- --nocapture --test-threads=1
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
