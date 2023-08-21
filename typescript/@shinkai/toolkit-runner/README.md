# Shinkai Tool Runner

The Shinkai Tool Runner can be used as either an HTTP server (production) or as an executable (for testing).

## Compilation

Before using the runner you first need to build it:

```
npm i
npm run build
```

## Exec Mode

```
node build/runner.js -h
Usage: runner [options]

Options:
  -e, --exec-mode              Execution mode: exec
  -w, --http-mode              Execution mode: http
  -s, --source <string>        For exec-mode, path to the source file
  -c, --get-config             For exec-mode, extract the config from the source file
  -v, --validate               For exec-mode, validate the headers
  -t, --tool <string>          For exec-mode, name of the tool to execute
  -i, --input <json-string>    For exec-mode, input data as a JSON string
  -x, --headers <json-string>  For exec-mode, headers as a JSON string
  -p, --port <number>          For http-mode, port to listen to (default: 3000)
  -h, --help                   display help for command
```

### Executing A Tool

**IMPORTANT** This will execute ANY provided source code on your local machine. Thus please ensure that you only run toolkits from known trusted sources (unless you are on a sandboxed machine).

This method is intended for unit testing and development.

```
node build/runner.js -e -s packaged-shinkai-toolkit.js -t isEven -i '{"number": 2}'
```

Response:

```
> {"isEven":true}
```

### Validate Toolkit Headers

You can execute the toolkit's internal `validateHeaders()` function which validates that the provided API keys (or other headers) are accepted by the required services and work.

```
node build/runner.js -e -v -s packaged-shinkai-toolkit.js -x '{ "x-shinkai-my-header": "my-api-key" }'
```

Response:

```
> true
```

### Generate Toolkit JSON:

You can generate the Toolkit JSON from the packaged toolkit using the following:

```
node build/runner.js -e -s packaged-shinkai-toolkit.js -c
```

Response:

```
> `{
> "toolkit-name": "@shinkai/toolkit-example",
> "author": "shinkai-dev",
> "version": "0.0.1",
> "executionSetup": {},
> ...
> }
```

## Webserver Mode (HTTP)

In webserver mode, the toolkit runner offers applications (like the Shinkai node or otherwise) the ability to easily execute tools by providing all data through HTTP requests.

Of note, the runner in webserver mode is meant to run sandboxed and not be publicly accessible as it executes whatever code is within the toolkit. Be careful, when using the runner outside of the Shinkai node in production.

To start the runner in webserver mode on port 3000, simply do:

```
node build/runner.js -w -p 3000
```

### Tool Execution - POST `/exec`

This endpoint runs the `tool`, from the provided `source` JS packaged toolkit, using the given `input` json, with the supplied headers.

#### JSON Data Fields:

- `tool`: Tool Name e.g., `"isEven"`
- `input`: Tool Input Data JSON: e.g., `{ "number": 2 }`
- `source`: Full Packaged Toolkit JS string e.g., `"(() => { // webpackBootstrrap\n var \_\_webpack_modules\_\_ = ({..."`

#### Headers:

- `x-shinkai-*`: Custom Fields

#### Example Request

```
curl localhost:3000/exec -H "Content-Type: application/json" -d @run-is-even.json
```

Response:

```
> {"isEven":true}
```

### Validate Toolkit Headers - POST `/validate`

Executes the toolkit's internal `validateHeaders()` function which validates that the provided API keys (or other headers) are accepted by the required services and work.

#### JSON Data Fields:

- `source` : Full Packaged Toolkit JS string e.g., `"(() => { // webpackBootstrrap\n var \_\_webpack_modules\_\_ = ({..."`

#### Headers:

- `x-shinkai-*`: Custom Fields

#### Example Request

```
curl localhost:3000/validate -H "Content-Type: application/json" -d @validate.json
```

Response:

```
> true
```

### Generate Toolkit JSON - POST `/toolkit_json`

#### JSON Data Fields:

- `source` : Full Packaged Toolkit JS string e.g., `"(() => { // webpackBootstrrap\n var \_\_webpack_modules\_\_ = ({..."`

#### Example Request

```
curl localhost:3000/config -H "Content-Type: application/json" -d @run-is-even.json
```

Response:

```
> `{
> "toolkit-name": "@shinkai/toolkit-example",
> "author": "shinkai-dev",
> "version": "0.0.1",
> "executionSetup": {},
> ...
> }
```
