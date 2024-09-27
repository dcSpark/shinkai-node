<h1 align="center">
  <img src="files/icon.png"/><br/>
  Shinkai Node
</h1>
<p align="center">The Shinkai Node is the central unit within the Shinkai Network that links user devices and oversees AI agents. Its diverse functions include processing user inputs, managing AI models, handling external containerized tooling for AI, coordinating computing tasks, generating proofs, converting and indexing data into vector embeddings, and ensuring efficient task execution according to user needs. The nodes play a crucial role in maintaining the network's decentralized structure, which enhances both security and privacy.<br/><br/> There is a companion repo called Shinkai Apps, that allows you to locally run the node and also easily manage AI models using Ollama, you can find it <a href="https://github.com/dcSpark/shinkai-apps">here</a>.</p><br/>

[![Mutable.ai Auto Wiki](https://img.shields.io/badge/Auto_Wiki-Mutable.ai-blue)](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Documentation

General Documentation: [https://docs.shinkai.com](https://docs.shinkai.com)

More In Depth Codebase Documentation (Mutable.ai): [https://wiki.mutable.ai/dcSpark/shinkai-node](https://wiki.mutable.ai/dcSpark/shinkai-node)

## Requirements

### Rust

The Shinkai Node requires Rust 1.76.0 or later.

### GCC Compiler Setup

Make sure you have gcc/g++ as your default compilers:

```
export CC=gcc
export CXX=g++
```


### Protobuf Compiler

The Shinkai Node requires the `protobuf-compiler` library. Here are the installation instructions for different operating systems:

#### Windows

1. Download the latest release from the [official protobuf releases page](https://github.com/protocolbuffers/protobuf/releases).
2. Extract the downloaded zip file.
3. Add the `bin` directory to your system's PATH environment variable.

#### Linux

For Debian-based distributions (e.g., Ubuntu):

```
sudo apt-get install -y protobuf-compiler
```

For Fedora:

```
sudo dnf install -y protobuf-compiler
```

For Arch Linux:

```
sudo pacman -S protobuf
```

#### MacOS

```
brew install protobuf
```

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

### Testing All Sub-projects Locally

Simply run the following command to run tests for all projects:

```
sh scripts/test_all_subprojects.sh
```

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

#### WASM tests

```
# Build testing image - shinkai-message-wasm
docker build -t testing_image_wasm -f .github/Dockerfile.wasm .

# Run tests shinkai-message-wasm cargo tests
docker run --entrypoint /entrypoints/run-wasm-pack-tests.sh testing_image_wasm

# Run tests shinkai-message-wasm wasm-pack tests
docker run --entrypoint /entrypoints/run-wasm-cargo-tests.sh testing_image_wasm
```

### Shinkai App tests

You need to compile the wasm library from `shinkai-message-wasm` and copy the resulting `pkg` to `shinkai-app/src/pkg` (if a folder already exists you should delete first). Then you will be able to run the tests inside `shinkai-app`

```
npm run test.unit
```

### Shinkai PYO3 Tests

### Further CI Development

Use `act -j test-wasm -P self-hosted=nektos/act-environments-ubuntu:18.04 --container-architecture linux/amd64` to run the tests locally in a docker container. This is useful for debugging CI issues.

## Releasing a New Version

When releasing a new version, ensure that you update the `Cargo.toml` of the shinkai-node as well as the `Cargo.toml` of the shinkai-libs/shinkai-http-api library.