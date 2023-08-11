# Shinkai-Node: Typescript

Contents

## shinkai-toolkit-buidler

To create and build a toolkit:
```
# Node.js 16+ is required
./shinkai-toolkit-buidler/new-tool.js MyToolName

cd MyToolName
npm i
npm run build
npm run test
```

This creates a new folder called `MyToolName` with a base project template.

The compiled toolkit is located at `dist/packaged-shinkai-toolkit.js`

## shinkai-toolkit-example

This is an DEMO toolkit that contains tools:
* isEven: detect if a number is even or not 
* HTTP: perform a http request

```
cd shinkai-toolkit-example
npm ci
npm run build
```
The compiled toolkit is located at `dist/packaged-shinkai-toolkit.js`


## shinkai-toolkit-runner

This is the program that executes compiled `packaged-shinkai-toolkit.js` 

You have two options for using the runner, either as an executable or webserver.

### As an executable:

```
cd shinkai-toolkit-runner
npm ci
npm run build
node build/runner.js -e -s ../shinkai-toolkit-example/dist/packaged-shinkai-toolkit.js -t isEven -i '{"number": 2}'
```

### As a webserver

```
cd shinkai-toolkit-runner
npm ci
npm run build
node build/runner.js -w
```

Trigger executing a tool inside of a toolkit via network request:
```
curl localhost:3000/exec -H "Content-Type: application/json" -d '{ {"tool":"isEven","input":{"number":2},"source":"<FILE CONTENTS>" }'`

# replace <FILE CONTENTS> with content of your given `packaged-shinkai-toolkit.js`
```