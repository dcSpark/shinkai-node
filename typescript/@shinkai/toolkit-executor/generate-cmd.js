/* eslint-disable no-process-exit */

// This is helper tool to build the JSON input for the executor web-mode.
//
// USAGE:
// node generate-cmd.js -o my_data.json -t isEven -i '{"number":2}' -p '../toolkit-example/dist/packaged-shinkai-toolkit.js'
//
// Then this output can be used to run the executor:
// curl -XPOST localhost:3002/execute_tool -H "content-type: application/json" -d @my_data.json
// curl -XPOST localhost:3002/validate_headers -H "content-type: application/json" -d @my_data.json
// curl -XPOST localhost:3002/toolkit_json -H "content-type: application/json" -d @my_data.json
//

const fs = require('fs');
const {parseArgs} = require('node:util');

const args = process.argv;
const options = {
  output: {
    type: 'string',
    short: 'o',
  },
  tool: {
    type: 'string',
    short: 't',
  },
  input: {
    type: 'string',
    short: 'i',
  },
  headers: {
    type: 'string',
    short: 'x',
  },
  packedShinkaiToolkit: {
    type: 'string',
    short: 'p',
  },
};

const {values} = parseArgs({
  args,
  options,
  allowPositionals: true,
});

const {input, headers, output, tool, packedShinkaiToolkit} = values;

// Mandatory fields.
if (!output) {
  console.log('No output specified (-o --output)');
  process.exit(1);
}
if (!tool) {
  console.log('No tool specified (-t --tool)');
  process.exit(1);
}

// Extract and default values.
let input_ = {};
if (input) {
  input_ = JSON.parse(input);
}
let headers_ = {};
if (headers) {
  headers_ = JSON.parse(headers);
}
let packagedShinkaiToolkit_ = './dist/packaged-shinkai-toolkit.js';
if (packedShinkaiToolkit) {
  packagedShinkaiToolkit_ = packedShinkaiToolkit;
}

// Read toolkit and build the body.
const source = fs.readFileSync(packagedShinkaiToolkit_, 'utf8');
const body = {source, headers: headers_, input: input_, tool};

let output_ = output;
if (!output.endsWith('.json')) {
  output_ = output + '.json';
}

fs.writeFileSync(output_, JSON.stringify(body), 'utf8');
console.log('✅ Wrote file to', output_);
