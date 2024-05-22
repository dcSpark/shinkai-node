# Shinkai Side Executor

## Building the Project

The project needs to link the pdfium static library which should be available as `libpdfium.a` in the pdfium directory. If you wish to build pdfium from source follow the steps in the next section.

To build the project use the following command:

```sh
PDFIUM_STATIC_LIB_PATH="path-to/pdfium-lib-directory" cargo build
```

For example:

```sh
PDFIUM_STATIC_LIB_PATH="$(pwd)/pdfium/mac-x64" cargo build
```

### Building PDFium from source

In the `pdfium` directory adjust `target_os` to `mac|linux` and `target_cpu` to `x64|arm64`.

Run the script `build.sh` to pull pdfium from source and build the static library. After the script finishes `libpdfium.a` should be available in the same directory.

## Running the server

```sh
cargo run
```

### PDF parsing

To test the PDF parser make an HTTP multipart/form-data POST request to the `/v1/extract_json_to_text_groups` endpoint with a PDF file in the body such as:

```sh
curl -F "file=@/shinkai-node/files/shinkai_intro.pdf;filename=shinkai_intro.pdf" 127.0.0.1:8090/v1/extract_json_to_text_groups
```