const fs = require('fs');
const {execMode} = require('./../build/exec-mode');

process.env.LOG = 'false';

describe('Errors Exec', () => {
  test('fill-memory', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/errors/fill-memory.json', 'utf8'));
    const x = await execMode(data.source, data.tool, JSON.stringify(data.input), JSON.stringify(data.headers || {}));
    expect(x).toEqual({"outputs": {"error": "Worker terminated due to reaching memory limit: JS heap out of memory",}, "tool": "ErrorGenerator"});
  }, 60000);
});

describe('Errors Exec', () => {
  test('terminate', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/errors/terminate.json', 'utf8'));
    const x = await execMode(data.source, data.tool, JSON.stringify(data.input), JSON.stringify(data.headers || {}));
    expect(x).toEqual({"outputs": {"errorCode": 1,}, "tool": "ErrorGenerator"});
  }, 60000);
});

describe('Errors Exec', () => {
  test('throw-exception', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/errors/throw-exception.json', 'utf8'));
    const x = await execMode(data.source, data.tool, JSON.stringify(data.input), JSON.stringify(data.headers || {}));
    expect(x).toEqual({"outputs": {"error": "ErrorGenerator: throw-exception",}, "tool": "ErrorGenerator"});
  }, 60000);
});
