const {
  ShinkaiToolkitLib,
  GoogleCalendarQuickEvent,
} = require('./../dist/packaged-shinkai-toolkit');

describe('CreateQuick Event Test', () => {
  test('check object', async () => {
    // await ShinkaiToolkitLib.waitForLib();
    const config = await ShinkaiToolkitLib.emitConfig();

    expect(JSON.parse(config).tools[0].name).toEqual(
      new GoogleCalendarQuickEvent().constructor.name
    );
  });
});
