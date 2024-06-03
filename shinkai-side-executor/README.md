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

Run the follow script in the `pdfium` directory passing the `target_os` (`mac|linux`) and `target_cpu` (`x64|arm64`) as parameters:

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

- `--address`: The address the server will bind to. Default is `0.0.0.0:8090`.
- `--max-node-text-size`: Maximum length of the text a text group can hold. Default is `400` characters. Takes effect only if used with `parse_pdf` argument.
- `--parse-pdf`: Path to a PDF file to run PDF parser from the CLI. Specifying this parameter won't start the server.

## PDF parsing

### Using the server

To test the PDF parser make an HTTP multipart/form-data POST request to the `/v1/pdf/extract-to-text-groups/:max_node_text_size` endpoint with a PDF file in the body such as:

```sh
curl -F "file=@/shinkai-node/files/shinkai_intro.pdf;filename=shinkai_intro.pdf" 127.0.0.1:8090/v1/pdf/extract-to-text-groups/400
```

### Using the CLI

```sh
cargo run --release -- --parse-pdf=<PDF_FILE> --max-node-text-size=<SIZE> > result.json
```

## Running tests

```sh
PDFIUM_STATIC_LIB_PATH="path-to/pdfium-lib-directory" cargo test
```