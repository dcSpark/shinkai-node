const {CompareNumbers, DecoratorsTools} = require('./../build/registry');

describe('CompareNumbers test', () => {

  test('Check if numbers equal', async () => {
    await DecoratorsTools.waitForLib();

    const x = {number: 2, numberToCompare: 2};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('EQ');
  });

  test('Check if number LT', async () => {
    await DecoratorsTools.waitForLib();

    const x = {number: 2, numberToCompare: 10};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('LT');
  });

  test('Check if number GT', async () => {
    await DecoratorsTools.waitForLib();

    const x = {number: 30, numberToCompare: 1};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('GT');
  });
});
