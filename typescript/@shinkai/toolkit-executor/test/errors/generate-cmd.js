/* eslint-disable no-process-exit */
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
};

const {values, positionals} = parseArgs({
  args,
  options,
  allowPositionals: true,
});

const {input, headers, output, tool} = values;

if (!output) {
  console.log('No output specified (-o --output)');
  process.exit(1);
}
if (!tool) {
  console.log('No tool specified (-t --tool)');
  process.exit(1);
}
let input_ = {};
if (input) {
  input_ = JSON.parse(input);
}
let headers_ = {};
if (headers) {
  headers_ = JSON.parse(headers);
}

const source = fs.readFileSync('./dist/packaged-shinkai-toolkit.js', 'utf8');
const body = {source, headers: headers_, input: input_, tool};

let output_ = output;
if (!output.endsWith('.json')) {
  output_ = output + '.json';
}

fs.writeFileSync(output_, JSON.stringify(body), 'utf8');
console.log('✅ Wrote file to', output_);
