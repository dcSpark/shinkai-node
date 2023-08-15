const {isEven, isEvenInput} = require('./../build/packages/is-even');

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

describe('isEven Test', () => {
  test('check if number is even', async () => {
    await wait();
    const x = {number: 2};
    const result = await new isEven().run(x);
    expect(result).toEqual({isEven: true});
  });

  test('check if number is not even', async () => {
    await wait();
    const x = {number: 3};
    const result = await new isEven().run(x);
    expect(result).toEqual({isEven: false});
  });
});
