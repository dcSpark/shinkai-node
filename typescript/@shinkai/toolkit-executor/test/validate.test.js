const fs = require('fs');

const {validate} = require('./../build/exec-mode');

process.env.LOG = 'false';

describe('Test Runner: Validate', () => {
  test('Validate negative', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/header.json', 'utf8'));
    const x = await validate(data.source, JSON.stringify(data.headers || {}));
    expect(x).toEqual({"result": { "error": "ValidationError: \"example\" is required" } });
  });

  test('Validate positive', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/header.json', 'utf8'));
    const x = await validate(data.source, JSON.stringify({ 'x-shinkai-example': 'true' }));
    expect(x).toEqual({"result": true });
  });

  test('Type error', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/header.json', 'utf8'));
    const x = await validate(data.source, JSON.stringify({ 'x-shinkai-example': '1' }));
    expect(x).toEqual({"result": {"error": "ValidationError: \"example\" must be a boolean" } });
  });
});
