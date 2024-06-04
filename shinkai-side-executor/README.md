# Shinkai Side Executor

## Building the Project

The project needs to link the pdfium static library which should be available as `libpdfium.a` in the pdfium directory. If you wish to build pdfium from source follow the steps in the *Building PDFium from source* section.

To build the project use the following command:

```sh
PDFIUM_STATIC_LIB_PATH="path-to/pdfium-lib-directory" cargo build --release
```

For example:

```sh
PDFIUM_STATIC_LIB_PATH="$(pwd)/pdfium/mac-x64" cargo build --release
```

**Note**: If you encounter linker errors run `cargo clean` in the root and in the side executor project, then rebuild the project.

### Building PDFium from source

[Prerequisites](https://pdfium.googlesource.com/pdfium/)

Run the follow script in the `pdfium` directory passing the `target_os` (`linux|mac|win`) and `target_cpu` (`arm64|x64`) as parameters:

```sh
./build.sh os cpu
```

After the script finishes `libpdfium.a` should be available in the `$OS-$CPU` directory.

### Building with dynamic linking

[Dynamic library release](https://github.com/bblanchon/pdfium-binaries/releases)

On Windows run the following commands:

```sh
SET PDFIUM_DYNAMIC_LIB_PATH=<path-to-DLL-directory>
cargo build --release --no-default-features
```

## Downloading Ocrs models

To download models in .rten format run:

```sh
cd ocrs
./download-models.sh
```

`.rten` files should be downloaded in the `ocrs` folder.

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
PDFIUM_STATIC_LIB_PATH="path-to/pdfium-lib-directory" cargo test
```