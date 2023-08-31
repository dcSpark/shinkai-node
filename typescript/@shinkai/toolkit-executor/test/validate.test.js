const fs = require('fs');

const {validate} = require('./../build/exec-mode');

process.env.LOG = 'false';
/** 
 * Toolkit validation used.
 * 
  toolkitHeaders = [
    {
      name: 'example',
      description: 'Example Header',
      type: DATA_TYPES.BOOLEAN,
    },
  ];

  validateHeaders(headers: Record<string, string>): Promise<boolean> {
    console.log(headers);
    if (String(headers['example']) === String(false)) {
      throw new Error('Invalid value for example header');
    }
    if (String(headers['example']) === String(true)) {
      return Promise.resolve(true);
    }
    return Promise.resolve(false);
  }
 */

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

  test('Throw error', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/header.json', 'utf8'));
    const x = await validate(data.source, JSON.stringify({ 'x-shinkai-example': 'false' }));
    expect(x).toEqual({"result": {"error": "Invalid value for example header" } });
  });
});
