const x = require('./../dist');

class Lib {
  constructor() {
    this['toolkitName'] = 'Lib';
  }
}
x.isToolKit(Lib);

describe('toolkit lib', () => {
  test('Test', async () => {
    const generatedSetup = await x.ShinkaiToolkitLib.emitConfig();
    const setup = {toolkitName: 'Lib', toolkitHeaders: {}, tools: []};
    expect(generatedSetup).toEqual(JSON.stringify(setup, null, 2));
  });
});
