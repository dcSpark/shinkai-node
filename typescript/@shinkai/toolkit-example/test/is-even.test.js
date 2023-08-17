const {isEven} = require('./../build/registry');

describe('Is Even test', () => {

  test('2 is even', async () => {
    const x = {number:2};
    const result = await new isEven().run(x);
    expect(result.isEven).toEqual(true);
  });

  test('3 is not even', async () => {
    const x = {number:3};
    const result = await new isEven().run(x);
    expect(result.isEven).toEqual(false);
  });
});
