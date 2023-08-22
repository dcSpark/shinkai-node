# Shinkai-Node: Typescript

## Summary

The @shinkai namespace contains:

- Toolkits
  - `@shinkai/toolkit-web`
  - `@shinkai/toolkit-smtp`
  - `@shinkai/toolkit-gmail`
  - `@shinkai/toolkit-goolge-calendar`
- Example Toolkit
  - `@shinkai/toolkit-example`
- Runner - Executor
  - `@shinkai/toolkit-executor`
- Libraries
  - `@shinkai/toolkit-lib`
  - `@shinkai/builder`

# @shinkai/toolkit-buidler

The builder scaffolds a new toolkit.

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

The compiled toolkit is located at `./MyToolName/dist/packaged-shinkai-toolkit.js`

# @shinkai/toolkit-example

This is a DEMO toolkit that contains tools:

- isEven: detect if a number is even or not.
- isInteger: detect if a number is an integer or not.

```
cd shinkai-toolkit-example
npm ci
npm run build
```

The compiled toolkit is located at `@shinkai/toolkit-example/dist/packaged-shinkai-toolkit.js`

# @shinkai/toolkit-executor

This is the program that executes compiled `packaged-shinkai-toolkit.js`

You have two options for using the executor, either as an executable or web server.
More information at `@shinkai/toolkit-executor/README.md`

## As an executable:

```
cd shinkai-toolkit-executor
npm ci
npm run build
node build/runner.js -e -s ../shinkai-toolkit-example/dist/packaged-shinkai-toolkit.js -t isEven -i '{"number": 2}'
```

## As a webserver

```
cd shinkai-toolkit-executor
npm ci
npm run build
node build/runner.js -w
```

Trigger executing a tool inside of a toolkit via network request:

```
curl localhost:3000/exec -H "Content-Type: application/json" -d '{"tool":"isEven","input":{"number":2},"source":"<FILE CONTENTS>" }'
# replace <FILE CONTENTS> with content of your given `packaged-shinkai-toolkit.js`
```

# @shinkai/toolkit-lib

Internal core library for toolkits.

Installation: `npm install --save @shinkai/toolkit-lib`
This library provides introspection and interfaces.

# @shinkai/toolkit-\*

## `@shinkai/toolkit-web`

Implementation of common web tools such as HTTP-fetch.

## `@shinkai/toolkit-smtp`

Send emails via SMTP

## `@shinkai/toolkit-gmail`

Send emails via Gmail (Requires OAuth)

## `@shinkai/toolkit-goolge-calendar`

Create events in Google-Calendar (Requires OAuth)
