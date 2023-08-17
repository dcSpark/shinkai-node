const {CompareNumbers} = require('./../build/registry');

describe('CompareNumbers test', () => {

  test('Check if numbers equal', async () => {
    const x = {number: 2, numberToCompare: 2};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('EQ');
  });

  test('Check if number LT', async () => {
    const x = {number: 2, numberToCompare: 10};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('LT');
  });

  test('Check if number GT', async () => {
    const x = {number: 30, numberToCompare: 1};
    const result = await new CompareNumbers().run(x);
    expect(result.comparison).toEqual('GT');
  });
});
