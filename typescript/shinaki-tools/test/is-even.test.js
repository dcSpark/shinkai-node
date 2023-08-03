const {isEven} = require('./../build/packages/is-even');

describe('isEven Test', () => {
  test('check if number is even', async () => {
    const x = {number: 2};
    const result = await new isEven().run(x);
    expect(result).toEqual({isEven: true});
  });
});
