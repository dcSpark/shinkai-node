const {Sample} = require('./../build/packages/sample');

describe('isInteger: "2"', () => {
  test('check if number is integer', async () => {
    const x = {number: '2'};
    const result = await new Sample().run(x);
    expect(result.isInteger).toEqual(true);
  });
});

describe('isInteger: 2 ', () => {
  test('check if number is integer', async () => {
    const x = {number: 2};
    const result = await new Sample().run(x);
    expect(result.isInteger).toEqual(true);
  });
});

describe('isInteger: Potato', () => {
  test('check if number is integer', async () => {
    const x = {number: 'Potato'};
    const result = await new Sample().run(x);
    expect(result.isInteger).toEqual(false);
  });
});
