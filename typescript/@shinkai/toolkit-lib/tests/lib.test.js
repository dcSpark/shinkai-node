const x = require('./../dist');

class Lib {
  constructor() {
    this['toolkit-name'] = 'Lib';
  }
}
x.isToolKit(Lib);

describe('toolkit lib', () => {
  test('Test', async () => {
    const generatedSetup = await x.ShinkaiTookitLib.emitConfig();
    const setup = {'toolkit-name': 'Lib', executionSetup: {}, tools: []};
    expect(generatedSetup).toEqual(JSON.stringify(setup, null, 2));
  });
});
