// import { PieceAuth, createPiece } from '@activepieces/pieces-framework';
import {createPiece} from '../../../@activespieces/pieces-framework';

import {createQuickCalendarEvent} from './lib/actions/create-quick-event';
import {calendarEventChanged} from './lib/triggers/calendar-event';
import {createEvent} from './lib/actions/create-event';
import {googleCalendarAuth} from './auth';

export const googleCalendar = createPiece({
  minimumSupportedRelease: '0.5.0',
  logoUrl: 'https://cdn.activepieces.com/pieces/google-calendar.png',
  displayName: 'Google Calendar',
  authors: ['osamahaikal', 'bibhuty-did-this'],
  auth: googleCalendarAuth,
  actions: [createQuickCalendarEvent, createEvent],
  triggers: [calendarEventChanged],
});
