const {HTTP} = require('./../build/registry');

describe('HTTP Test', () => {
  test('Check if numbers equal', async () => {
    const x = {
      url: 'https://docs.google.com/spreadsheets/d/e/2PACX-1vRjBOL2YIe8UOCD-ssFgRuemzGY-SO-j5llsDP7HYzNwaWyg5g2Ou5DzojNtQfTVwmcw4wMbjlOhE2N/pub?gid=0&single=true&output=csv',
      method: 'get',
    };
    const result = await new HTTP().run(x);
    const response = result.response.replace(/[^a-z0-9,]/g, '');
    expect(response).toEqual(`this,1is,2a,3sample,4file,5`);
  });
});
