# Shinkai PDF Parser

## Building the Project

To build the project use the following command:

```sh
cargo build --release
```

**Note**: Build the project in release mode or try to prevent running `ocrs` in debug mode since it will be extremely slow.

Alternatively embed debug info to the release build by running:

```sh
RUSTFLAGS=-g cargo build --release
```

### Static linking PDFium

By default the project binds to the PDFium dynamic library at runtime. To statically link PDFium build with feature `static` enabled:

```sh
cargo build --release --features static
```

The project needs to link the PDFium static library which should be available as `libpdfium.a` in the PDFium directory. If you wish to build PDFium from source follow the steps in the *Building PDFium static library from source* section.

**Note**: If you encounter linker errors run `cargo clean` in the root directory then rebuild the project.

### Building PDFium static library from source

[Prerequisites](https://pdfium.googlesource.com/pdfium/)

Run the follow script in the `pdfium` directory passing the `target_os` (`linux|mac|win`) and `target_cpu` (`arm64|x64`) as parameters to produce the static library:

```sh
./build.sh os cpu
```

After the script finishes `libpdfium.a` should be available in the `$OS-$CPU` directory.

#### Using docker

To build the library on Linux step into the `pdfium` directory and build the image:

```sh
docker build -t build-pdfium -f Dockerfile .
```

Mount directory `linux-x64` and run the container:

```sh
docker run -v $(PWD)/linux-x64:/app/linux-x64 --name build-pdfium build-pdfium
```

## Downloading Ocrs models

To download models in .rten format run:

```sh
cd ocrs && ./download-models.sh
```

`.rten` files should be downloaded in the `ocrs` folder.

## Cargo run and test with dynamic linking

[Dynamic library releases](https://github.com/bblanchon/pdfium-binaries/releases)

Set `PDFIUM_DYNAMIC_LIB_PATH` environment variable to overwrite the default location directory of the library which is `pdfium/$OS-$CPU`.

```sh
PDFIUM_DYNAMIC_LIB_PATH=$(PWD)/pdfium/linux-x64 cargo test -- --test-threads=1
```

## Running tests

```sh
cargo test --features static -- --test-threads=1
```