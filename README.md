<h1 align="center">
  <img src="files/icon.png"/><br/>
  Shinkai Node
</h1>
<p align="center">Shinkai allows you to create AI agents without touching code. Define tasks, schedule actions, and let Shinkai write custom code for you. Native crypto support included.<br/><br/> There is a companion repo called Shinkai Apps which contains the frontend that encapsulates this project, you can find it <a href="https://github.com/dcSpark/shinkai-apps">here</a>.</p><br/>

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
