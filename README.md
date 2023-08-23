# Shinkai Node

## Dependencies

### MuPDF

Linux:

```
sudo apt install mupdf libfontconfig1-dev gcc g++
```

Mac:

```
brew install mupdf fontconfig
```

### GCC Compiler Setup

Make sure you have gcc/g++ as your default compilers:

```
export CC=gcc
export CXX=g++
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


