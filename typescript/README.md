# Shinkai-Node: Typescript

Contents

## shinkai-toolkit-buidler

To create new toolkit, run:

`./shinkai-toolkit-buidler MyToolName`

This creates a new folder called `MyToolName` with a empty project.

To create and build Tool:
```
# Node.js 16+ is required
./shinkai-toolkit-buidler/new-tool.js MyToolName

cd MyToolName
npm i
npm run build
npm run test
```

The compiled toolkit is located at `dist/shinkai-tools.js`

## shinkai-toolkit-example

This is an DEMO toolkit that contains tools:
* isEven: detect if a number is even or not 
* HTTP: perform a http request

```
cd shinkai-toolkit-example
npm ci
npm run build
```
The compiled toolkit is located at `dist/shinkai-tools.js`


## shinkai-toolkit-runner

This is program that executes the compiled `shinkai-tools.js` 
* Runs as an executable:

```
cd shinkai-toolkit-runner
npm ci
npm run build
node build/runner.js -e -s ../shinkai-toolkit-example/dist/shinkai-tools.js -t isEven -i '{"number": 2}'
```

* Runs as a webserver

```
cd shinkai-tookkit-runner
npm ci
npm run build
node build/runner.js -w
```

Perform a network request
```
curl localhost:3000/exec -H "Content-Type: application/json" -d '{ {"tool":"isEven","input":{"number":2},"source":"<FILE CONTENTS>" }'`

# replace <FILE CONTENTS> with content of shinkai-toolkit-example/dist/shinkai-tools.js
```