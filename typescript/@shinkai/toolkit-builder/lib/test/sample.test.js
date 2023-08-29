const {ShinkaiToolkitLib, Sample} = require('./../build/registry');

describe('Sample test', () => {
  test('Check if numbers equal', async () => {
    await ShinkaiToolkitLib.waitForLib();

    const x = {number: 2, numberToCompare: 2};
    const result = await new Sample().run(x);
    expect(result.comparison).toEqual('EQ');
  });

  test('Check if number LT', async () => {
    await ShinkaiToolkitLib.waitForLib();

    const x = {number: 2, numberToCompare: 10};
    const result = await new Sample().run(x);
    expect(result.comparison).toEqual('LT');
  });

  test('Check if number GT', async () => {
    await ShinkaiToolkitLib.waitForLib();

    const x = {number: 30, numberToCompare: 1};
    const result = await new Sample().run(x);
    expect(result.comparison).toEqual('GT');
  });
});
