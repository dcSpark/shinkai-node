const fs = require('fs');

const {toolkitConfig, validate, execMode} = require('./../build/exec-mode');

process.env.LOG = 'false';

describe('Test Runner: Exec', () => {
  test('Help', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/data.json', 'utf8'));
    const x = await execMode(data.source, data.tool, JSON.stringify(data.input), JSON.stringify(data.headers || {}));
    expect(x).toEqual({"outputs":  [{
        "description": "Result of the check. True if the number is even.",
        "ebnf": "(\"true\"|\"false\")",
        "enum": undefined,
        "isOptional": false,
        "name": "isEvenOutput",
        "result": true,
        "type": "BOOL",
        "wrapperType": "none",
      },
    ], "tool": "isEven"});
  });
});

describe('Test Runner: Validate', () => {
  test('Help', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/data.json', 'utf8'));
    const x = await validate(data.source, JSON.stringify(data.headers || {}));
    expect(x).toEqual({"result": true });
  });
});

describe('Test Runner: Config', () => {
  test('Help', async () => {
    const data = JSON.parse(fs.readFileSync(__dirname + '/data/data.json', 'utf8'));
    const x = await toolkitConfig(data.source);
    expect(x.toolkitName).toEqual("toolkit-example");
  });
});
