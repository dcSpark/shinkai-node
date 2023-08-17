const {DecoratorsTools} = require('./../dist/packaged-shinkai-toolkit');

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

describe('CreateQuick Event Test', () => {
  test('check object', async () => {
    const config = await DecoratorsTools.emitConfig();
    expect(JSON.parse(config).tools[0].name).toEqual(
      'GoogleCalendarQuickEvent'
    );
  });
});
