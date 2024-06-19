# Shinkai Executor

## Building the Project

To build the project use the following command:

```sh
cargo build --release
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