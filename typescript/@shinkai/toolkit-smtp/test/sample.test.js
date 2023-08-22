const {SMTP, ShinkaiTookitLib} = require('./../dist/packaged-shinkai-toolkit.js');

describe('SMTP test', () => {

  test('Check input validator', async () => {
    await ShinkaiTookitLib.waitForLib();
    const input = {from:'aa', to:['bb'], subject:'cc', body:'dd'};
    const smtp = new SMTP();
    const result = await smtp.validateInputs(input);
    expect(result).toEqual({from:'aa', to:['bb'], subject:'cc', body:'dd'});
  });

});
