const {execModeConfig} = require('./../build/exec-mode');

describe('Test Runner', () => {
  test('Help', async () => {
    
    const x = await execModeConfig('./test/stub.js');
    expect(x).toEqual('Echo: stub.js\n');
  });

});