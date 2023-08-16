const {
  GoogleCalendarQuickEvent,
} = require('./../dist/packaged-shinkai-toolkit');

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

describe('CreateQuick Event Test', () => {
  test('check object', async () => {
    await wait();
    const result = await new GoogleCalendarQuickEvent();
    expect(result).toEqual({
      description: 'Activepieces Create Quick Event at Google Calendar',
    });
  });
});
