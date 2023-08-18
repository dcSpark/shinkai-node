# Shinkai Tool Runner

This tool can be run as an HTTP server or as an executable.

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

### Execute Tool:

This is the standard execution for testing tools.  
**IMPORTANT** This will execute ANY provided source code, please run only code from known sources.  
This is intended for unit testing and development.

`-e -s <source>`

EXAMPLE:

`node runner.js -e -s packaged-shinkai-toolkit.js -t isEven -i '{"number": 2}'`
> `{"isEven":true}`

### Validate Setup:

This will execute the internal `validateHeaders()` function that validates that API keys are OK and are accepted by the required services.  
`-e -s <source> -v`

EXAMPLE:

`node runner.js -e -s packaged-shinkai-toolkit.js -x '{ "x-shinkai-my-header": "TEST" }'`
> `true`

### Generate Toolkit Interface:
`-e -s <source> -c`

EXAMPLE:

`node runner.js -e -s packaged-shinkai-toolkit.js -c`  

> `{
  "toolkit-name": "@shinkai/toolkit-example",
  "author": "shinkai-dev",
  "version": "0.0.1",
  "executionSetup": {},
  "tools": [
    {
      "name": "CompareNumbers",
      "description": "Check if number is greater than, lower than or equal to another number.",
      "input": [
        {
          "name": "number",
          "type": "INT",
          "description": "Number to check if greater than, lower than or equal than.",
          "isOptional": false,
          "wrapperType": "none",
          "ebnf": "(-?[0-9]+)"
        },
        {
          "name": "numberToCompare",
          "type": "INT",
          "description": "Number to compare with.",
          "isOptional": false,
          "wrapperType": "none",
          "ebnf": "(-?[0-9]+)"
        }
      ],
      "output": [
        {
          "name": "comparison",
          "type": "ENUM",
          "description": "Result of the comparison.",
          "isOptional": false,
          "wrapperType": "none",
          "enum": [
            "GT",
            "LT",
            "EQ"
          ],
          "ebnf": "(\"GT\" | \"LT\" | \"EQ\")"
        }
      ],
      "inputEBNF": "number ::= (-?[0-9]+)\nnumberToCompare ::= (-?[0-9]+)\ncomparison ::= (\"GT\" | \"LT\" | \"EQ\")"
    },
    {
      "name": "isEven",
      "description": "Check if a number is even",
      "input": [
        {
          "name": "number",
          "type": "INT",
          "description": "Integer number to check if is even.",
          "isOptional": false,
          "wrapperType": "none",
          "ebnf": "(-?[0-9]+)"
        }
      ],
      "output": [
        {
          "name": "isEven",
          "type": "BOOL",
          "description": "Result of the check. True if the number is even.",
          "isOptional": false,
          "wrapperType": "none",
          "ebnf": "(\"true\"|\"false\")"
        }
      ],
      "inputEBNF": "number ::= (-?[0-9]+)\nnumberToCompare ::= (-?[0-9]+)\ncomparison ::= (\"GT\" | \"LT\" | \"EQ\")\nnumber ::= (-?[0-9]+)\nisEven ::= (\"true\"|\"false\")"
    }
  ]
}
`

## Http Mode

### Run server
`node runner.js -w`  

**IMPORTANT**: This is a simple server, meant to run in trusted networks as it executes custom code.

### Request Tool Execution
POST `json` @ /exec   

JSON Fields:
* `tool`: Tool Name e.g., isEven
* `input`: Tool Input Data: e.g., { "number": 2 }
* `source`: Full JS blob e.g., "(() => { // webpackBootstrrap\n var \_\_webpack_modules__ = ({..."

Headers:
* `x-shinkai-*`: Custom Fields

EXAMPLE:

`localhost:3000/exec -H "Content-Type: application/json" -d @run-is-even.json`
> `{"isEven":true}`

### Request Validate Setup
POST `json` @ /validate
JSON Fields
* `source` : Full JS blob e.g., "(() => { // webpackBootstrrap\n var \_\_webpack_modules__ = ({..."

Headers:
* `x-shinkai-*`: Custom Fields

EXAMPLE:

`localhost:3000/validate -H "Content-Type: application/json" -d @run-is-even.json`
> `true`

### Request Tool Interface
POST `json` @ /config  
JSON Fields:
* `source`: Full JS blob e.g., "(() => { // webpackBootstrrap\n var \_\_webpack_modules__ = ({..."

EXAMPLE:

`curl localhost:3000/config -H "Content-Type: application/json" -d @run-is-even.json`
> `{"toolkit-name":"Shinkai Toolkit","author":"local.shinkai","version":"0.0.1","tools":[{"name":"isEven","description":"Check if a number is even","input":[{"name":"number","type":"INT","description":"Integer number to check if is even.","isOptional":false,"wrapperType":"none"}],"output":[{"name":"isEven","type":"BOOL","description":"Result of the check. True if the number is even.","isOptional":false,"wrapperType":"none"}]}]}`

