# Shinkai Tool Runner

This tool can be run as a HTTP server or as a executable.

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

## Exec Mode

### Execute tool:

`node runner.js -e -s packaged-shinkai-toolkit.js -t isEven -i '{"number": 2}'`
> `{"isEven":true}`

### Generate interface:
`node runner.js -e -s packaged-shinkai-toolkit.js -c`
> `{
  "toolkit-name": "Shinkai Toolkit",
  "author": "local.shinkai",
  "version": "0.0.1",
  "tools": [
    {
      "name": "isEven",
      "description": "Check if a number is even",
      "input": [
        {
          "name": "number",
          "type": "INT",
          "description": "Integer number to check if is even.",
          "isOptional": false,
          "wrapperType": "none"
        }
      ],
      "output": [
        {
          "name": "isEven",
          "type": "BOOL",
          "description": "Result of the check. True if the number is even.",
          "isOptional": false,
          "wrapperType": "none"
        }
      ]
    }
  ]
}`

## Http Mode

### Run server
`node runner.js -w`  

IMPORTANT: This is simple server, meant to run in trusted networks as it executes custom code.

### Request Tool Execution
POST `json` @ /exec   

JSON Fields:
* `tool` : Tool Name e.g., isEven
* `input` : Tool Input Data: e.g., { "number": 2 }
* `source` : Full JS blob e.g., "(() => { // webpackBootstrrap\n var \_\_webpack_modules__ = ({..."


`localhost:3000/exec -H "Content-Type: application/json" -d @run-is-even.json`
> `{"isEven":true}`

### Request Tool 
POST `json` @ /config  
JSON Fields:
* `source` : Full JS blob e.g., "(() => { // webpackBootstrrap\n var \_\_webpack_modules__ = ({..."

`curl localhost:3000/config -H "Content-Type: application/json" -d @run-is-even.json`
> `{"toolkit-name":"Shinkai Toolkit","author":"local.shinkai","version":"0.0.1","tools":[{"name":"isEven","description":"Check if a number is even","input":[{"name":"number","type":"INT","description":"Integer number to check if is even.","isOptional":false,"wrapperType":"none"}],"output":[{"name":"isEven","type":"BOOL","description":"Result of the check. True if the number is even.","isOptional":false,"wrapperType":"none"}]}]}`

