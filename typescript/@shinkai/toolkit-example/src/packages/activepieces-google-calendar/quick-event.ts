import {
  isTool,
  output,
  input,
  isString,
  setup,
  isOptional,
  isEnum,
  BaseTool,
  BaseInput,
  BaseOutput,
  BaseSetup,
} from '@shinkai/toolkit-lib';
import {googleCalendar} from '../../lib/google-calendar/src/index';
import {Context} from '@activepieces/pieces-framework';

@input('GoogleCalendarQuickEvent')
class APGoogleCalendarInput extends BaseInput {
  @isString('Calendar ID. Primary calendar used if not specified')
  @isOptional
  calendar_id = 'primary';

  @isString('Summary: The text describing the event to be created')
  text!: string;

  @isEnum(
    ['all', 'externalOnly', 'none'],
    'Send Updates: Guests who should receive notifications about the creation of the new event.'
  )
  @isOptional
  send_updates = 'none';
}

@output('GoogleCalendarQuickEvent')
class APGoogleCalendarOutput extends BaseOutput {
  @isString('Network Response')
  response!: string;
}

@setup('GoogleCalendarQuickEvent')
class APGoogleCalendarSetup extends BaseSetup {
  @isString('OAuth Token.')
  'x-shinkai-oauth'!: string;
}
@isTool
export class GoogleCalendarQuickEvent extends BaseTool<
  APGoogleCalendarInput,
  APGoogleCalendarOutput,
  APGoogleCalendarSetup
> {
  description = 'Activepieces Create Quick Event at Google Calendar';
  oauth = Object.assign({}, googleCalendar.auth, {cloudOAuth: 'activepieces'});
  async run(
    input: APGoogleCalendarInput,
    headers: APGoogleCalendarSetup
  ): Promise<APGoogleCalendarOutput> {
    this.validate(input);
    const createQuickCalendarEvent = googleCalendar.actions[0];

    const setup: Context = {
      auth: {
        access_token: headers['x-shinkai-oauth'],
      },
      propsValue: {
        calendar_id: input.calendar_id,
        text: input.text,
        send_updates: input.send_updates,
      }
    };
    const response = await createQuickCalendarEvent.run(setup);
    const out = new APGoogleCalendarOutput();
    out.response = JSON.stringify(response.body);
    return out;
  }
}
