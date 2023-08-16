import {DATA_TYPES, ShinkaiSetup, isToolKit} from '@shinkai/toolkit-lib';
import {googleCalendar} from './lib/google-calendar/src';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  'toolkit-name' = 'Shinkai Toolkit';
  author = 'local.shinkai';
  version = '0.0.1';

  // Register OAuth
  oauth = Object.assign({}, googleCalendar.auth, {
    cloudOAuth: 'activepieces',
  });

  executionSetup = [
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
