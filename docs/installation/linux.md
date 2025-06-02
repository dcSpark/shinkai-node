# Quick Start Guide for Linux

## Prerequisites

To build this project, you will need the following:

- Rust
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

### 2. Install OpenSSL Development Libraries

#### Ubuntu/Debian
```bash
sudo apt update
sudo apt install pkg-config libssl-dev
```

#### Fedora
```bash
sudo dnf install pkg-config openssl-devel
```

#### Arch Linux
```bash
sudo pacman -S openssl
```

### 3. Manually Set Environment Variables (if needed)

If OpenSSL is not detected during the build, manually set the environment variables:

```bash
export OPENSSL_DIR=/usr/lib/ssl  # Adjust based on your system
export PKG_CONFIG_PATH=$OPENSSL_DIR/lib/pkgconfig
```

### 4. Build the Project

Once all dependencies are installed, build the project with:

```bash
cargo build
```