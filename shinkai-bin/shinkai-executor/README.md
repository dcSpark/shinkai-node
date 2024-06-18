# Shinkai Executor

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

By default the executor service binds to the dynamic library. To statically link PDFium build with feature `static` enabled:

```sh
cargo build --release --features static
```

The project needs to link the pdfium static library which should be available as `libpdfium.a` in the pdfium directory. If you wish to build pdfium from source follow the steps in the *Building PDFium static library from source* section.

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

Make sure the `PDFIUM_DYNAMIC_LIB_PATH` environment variable is set to the directory that contains the dynamic library when running `cargo run` or `cargo test` commands:

```sh
PDFIUM_DYNAMIC_LIB_PATH=$(PWD)/pdfium/linux-x64 cargo test -- --test-threads=1
```

## Running the server

```sh
cargo run --release -- --address <ADDRESS>
```

### Arguments

#### Server
- `--address`: The address the server will bind to. Default is `0.0.0.0:8090`.

#### CLI

```sh
cargo run --release -- --help
```

## API documentation

- OpenAPI schema: `openapi/openapi.yaml`. 
- Online Swagger editor: https://editor.swagger.io/

## PDF parsing

### Using the server

To test the PDF parser make an HTTP multipart/form-data POST request to the `/v1/pdf/extract-to-text-groups` endpoint with a PDF file in the body such as:

```sh
curl -F "file=@/shinkai-node/files/shinkai_intro.pdf;filename=shinkai_intro.pdf" 127.0.0.1:8090/v1/pdf/extract-to-text-groups
```

### Using the CLI

```sh
cargo run --release -- pdf extract-to-text-groups --file=<PDF_FILE> --max-node-text-size=<SIZE> > result.json
```

## Running tests

```sh
cargo test -- --test-threads=1
```