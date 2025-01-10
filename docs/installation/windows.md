# Quick Start Guide for Windows

## Prerequisites

To build this project on Windows, you will need the following:

- Rust
- Protobuf
- OpenSSL development libraries

## Installing Dependencies

### 1. Install Rust

Download and install Rust via `rustup` by running the following command in **PowerShell**:

```powershell
iwr https://sh.rustup.rs -UseBasicParsing | iex
```

Follow the prompts to install Rust and configure your environment.

Verify installation:

```powershell
rustc --version
```

### 2. Install Protobuf

1. Download the [Protobuf release for Windows](https://github.com/protocolbuffers/protobuf/releases).
2. Extract the downloaded files and add the `bin` directory to your system's `PATH`.

   For example, add `C:\path\to\protobuf\bin` to the `PATH` environment variable.

Verify installation:

```powershell
protoc --version
```

### 3. Install OpenSSL Development Libraries

On Windows, the easiest way to get OpenSSL is through [vcpkg](https://github.com/microsoft/vcpkg). Follow these steps:

1. Install [vcpkg](https://github.com/microsoft/vcpkg):

   ```powershell
   git clone https://github.com/microsoft/vcpkg.git
   .\vcpkg\bootstrap-vcpkg.bat
   ```

2. Install OpenSSL using vcpkg:

   ```powershell
   .\vcpkg\vcpkg install openssl:x64-windows
   ```

3. Set the `OPENSSL_DIR` environment variable to the vcpkg installation path:
   ```powershell
   $env:OPENSSL_DIR = "C:\path\to\vcpkg\installed\x64-windows"
   $env:PKG_CONFIG_PATH = "$env:OPENSSL_DIR\lib\pkgconfig"
   ```

### 4. Build the Project

Once all dependencies are installed, build the project with:

```powershell
cargo build
```

### 2. Alternative: Using Scoop (Recommended)

Scoop provides a simpler way to install and manage development tools on Windows.

1. Install Scoop by running the following in PowerShell:

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex
```

2. Add required buckets and install dependencies:

```powershell
scoop bucket add main
scoop bucket add extras
scoop install rust llvm openssl
```

3. Install Visual Studio Build Tools:
   - Download the Build Tools installer from [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Run the installer and select "Desktop development with C++"
   - Wait for the installation to complete (this may take a while)

4. Set required environment variables:

```powershell
$env:OPENSSL_DIR = "$(scoop prefix openssl)"
$env:OPENSSL_ROOT_DIR = "$(scoop prefix openssl)"
[Environment]::SetEnvironmentVariable('OPENSSL_DIR', $env:OPENSSL_DIR, 'User')
[Environment]::SetEnvironmentVariable('OPENSSL_ROOT_DIR', $env:OPENSSL_ROOT_DIR, 'User')
```

5. Restart your terminal, then run:
```powershell
rustup default stable-msvc
```

Verify installations:
```powershell
clang --version
cargo --version
```

### 3. Alternative: Using WSL

For a smoother developer experience, you can also setup WSL (Windows Subsystem for Linux) and install a Linux distribution.

#### 1. Install WSL

Run the following command to install Ubuntu as the default distribution:

```powershell
wsl --install
```

Or run the following command to change the distribution installed:

```powershell
wsl --install -d <Distribution Name>
```

#### 2. Install Shinkai Node

Enter the following command to step inside the installed Linux distribution:

```powershell
wsl
```

Follow the instructions in the **Quick Start Guide for Linux** documentation to install Shinkai Node in a Linux environment.
