import {
  isTool,
  output,
  input,
  isBoolean,
  isInteger,
  isString,
} from '../../shinkai/Decortors';
import {BaseTool, BaseInput, BaseOutput} from '../../shinkai/BaseTool';
import {createQuickCalendarEvent} from './activepieces-original/google-calendar/src/lib/actions/create-quick-event';
import {Context} from './@activespieces/pieces-framework';

@input('ActivePiecesGoogleCalendar')
class APGoogleCalendarInput extends BaseInput {
  // TOOD DUPLICATED KEY NAMES DO NOT GET PROCESSED
  @isInteger('Integer number to check if is even.')
  number!: number;
}

@output('ActivePiecesGoogleCalendar')
class APGoogleCalendarOutput extends BaseOutput {
  // TOOD DUPLICATED KEY NAMES DO NOT GET PROCESSED
  @isString('Network Response')
  response!: string;
}

@isTool
export class ActivePiecesGoogleCalendar extends BaseTool<
  APGoogleCalendarInput,
  APGoogleCalendarOutput
> {
  description = 'Activepieces Google-Calendar';

  async run(input: APGoogleCalendarInput): Promise<APGoogleCalendarOutput> {
    this.validate(input);
    const _createQuickCalendarEvent = createQuickCalendarEvent;

    const setup: Context = {
      auth: {
        access_token:
          'ya29.a0AfB_byC03BHD4uADbA4LgXAnzSbUkaGb_K_DyyMR59GiTNFqcId237Pg_tDvMqsuxXikTRBGgwyiBNKAkmom4UWbdf2PFAU3CqGnawI1dgn9ZERToIrgBEIbNHNVpKGBH6ak1nkXT5GCkRsdTRA7BSp4KLlxaCgYKAfgSARASFQHsvYlswUZMh9jTtdnxwXUrddIMWg0163',
      },
      propsValue: {
        calendar_id: 'primary',
        text: 'Test from shinkai-tools!',
        send_updates: 'none',
      },
      webhookUrl: '',
    };
    const response = await _createQuickCalendarEvent.run(setup);
    const out = new APGoogleCalendarOutput();
    out.response = JSON.stringify(response.body);
    return out;
  }
}
