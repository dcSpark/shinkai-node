# Shinkai Vector Resources

## Tests

### GCC Compiler Setup

Make sure you have gcc/g++ as your default compilers in order to ensure bert.cpp can be compiled:

```
export CC=gcc
export CXX=g++
```

### Running Tests

The build.rs will initially run on first build, downloading the SBERT all-MiniLM model and bert.cpp (which gets compiled into an output bert-cpp-server).

For running the actual tests, you need to make sure to have them run sequentially (as each test orchestrates it's own bert.cpp process at the same port for simplicity). As such simply run:

```
cargo test -- --test-threads=1
```
