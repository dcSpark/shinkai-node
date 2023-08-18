# Shinkai Node

## Dependencies

For MuPDF you will need to install the following:

```
sudo apt install mupdf libfontconfig1-dev gcc g++
```

Mac:

```
brew install mupdf fontconfig
```

And make sure you have gcc/g++ as your default compilers:

```
export CC=gcc
export CXX=g++
```

## Tests

Note: You must run tests from the root directory of this repo.

- Use `cargo test -- --test-threads=1` to ensure all tests pass. This runs tests in sequence rather than in parallel.

- Use `cargo test tcp_node_test -- --nocapture --test-threads=1` to run one test and send output to console. Useful for debugging.



## Running tests locally

### Main tests
```
# Build testing image
docker build -t testing_image -f .github/Dockerfile .

# Run tests main cargo tests
docker run --entrypoint /entrypoints/run-main-cargo-tests.sh testing_image
```

### WASM tests
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


