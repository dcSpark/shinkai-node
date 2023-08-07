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
