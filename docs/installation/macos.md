# Quick Start Guide for macOS

## Prerequisites

To build this project on macOS, you will need the following:

- Rust
- Protobuf
- OpenSSL development libraries

## Installing Dependencies

### 1. Install Rust

Run the following command to install Rust via `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, configure your shell with:

```bash
source $HOME/.cargo/env
```

Verify installation:

```bash
rustc --version
```

### 2. Install Protobuf

To install Protobuf, use [Homebrew](https://brew.sh/):

```bash
brew install protobuf
```

Verify the installation:

```bash
protoc --version
```

### 3. Install OpenSSL Development Libraries

To install OpenSSL, use Homebrew:

```bash
brew install openssl
```

You may need to manually set environment variables to ensure the system finds the OpenSSL library:

```bash
export OPENSSL_DIR=$(brew --prefix openssl)
export PKG_CONFIG_PATH=$OPENSSL_DIR/lib/pkgconfig
```

### 4. Build the Project

Once all dependencies are installed, build the project with:

```bash
cargo build
```