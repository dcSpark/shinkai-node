import {
  DATA_TYPES,
  SHINKAI_OAUTH,
  ShinkaiSetup,
  isToolKit,
} from '@shinkai/toolkit-lib';
import {googleCalendar} from './lib/google-calendar/src';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  'toolkit-name' = 'Shinkai Toolkit';
  author = 'local.shinkai';
  version = '0.0.1';

  executionSetup = [
    // Register OAuth
    {
      name: SHINKAI_OAUTH,
      oauth: Object.assign({}, googleCalendar.auth, {
        cloudOAuth: 'activepieces',
      }),
    },
    // Register Auth & Keys
    {
      name: 'API_KEY',
      description: 'Some Optional API Key',
      type: DATA_TYPES.STRING,
      isOptional: true,
    },
    {
      name: 'API_SECRET',
      description: 'Api Secret key',
      type: DATA_TYPES.STRING,
    },
    {
      name: 'BASE_URL',
      description: 'Base URL for api',
      type: DATA_TYPES.STRING,
    },
  ];
}
