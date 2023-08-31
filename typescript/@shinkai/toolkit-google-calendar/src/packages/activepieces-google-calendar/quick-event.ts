import {
  isTool,
  isOutput,
  isInput,
  isString,
  isOptional,
  isEnum,
  BaseTool,
  BaseInput,
  BaseOutput,
  SHINKAI_OAUTH,
} from '@shinkai/toolkit-lib';
import { googleCalendar } from '../../lib/google-calendar/src/index';
import { Context } from '@activepieces/pieces-framework';

@isInput('GoogleCalendarQuickEvent')
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
  send_updates: 'none' | 'all' | 'externalOnly' = 'none';
}

@isOutput('GoogleCalendarQuickEvent')
class APGoogleCalendarOutput extends BaseOutput {
  @isString('Network Response')
  response!: string;
}

@isTool
export class GoogleCalendarQuickEvent extends BaseTool<
  APGoogleCalendarInput,
  APGoogleCalendarOutput
> {
  description = 'Activepieces Create Quick Event at Google Calendar';
  async run(
    input: APGoogleCalendarInput,
    headers: Record<string, string>
  ): Promise<APGoogleCalendarOutput> {
    const createQuickCalendarEvent = googleCalendar.actions[0];

    const setup: Context = {
      auth: {
        access_token: headers['oauth'] || headers[SHINKAI_OAUTH],
      },
      propsValue: {
        calendar_id: input.calendar_id,
        text: input.text,
        send_updates: input.send_updates,
      },
    };
    const response = await createQuickCalendarEvent.run(setup);
    const out = new APGoogleCalendarOutput();
    out.response = JSON.stringify(response.body);
    return out;
  }
}
