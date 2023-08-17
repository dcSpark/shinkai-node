const {DecoratorsTools, GoogleCalendarQuickEvent} = require('./../dist/packaged-shinkai-toolkit');

describe('CreateQuick Event Test', () => {
  test('check object', async () => {
    // await DecoratorsTools.waitForLib();
    const config = await DecoratorsTools.emitConfig();
    
    expect(JSON.parse(config).tools[0].name).toEqual(
      new GoogleCalendarQuickEvent().name
    );
  });
});
