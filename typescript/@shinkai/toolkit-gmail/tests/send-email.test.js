const {
  ShinkaiToolkitLib,
  GmailSendEmail,
} = require('./../dist/packaged-shinkai-toolkit');

describe('CreateQuick Event Test', () => {
  test('check object', async () => {
    // await ShinkaiToolkitLib.waitForLib();
    const config = await ShinkaiToolkitLib.emitConfig();

    expect(config.tools[0].name).toEqual(
      new GmailSendEmail().constructor.name
    );
  });
});
