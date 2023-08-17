import {DATA_TYPES, ShinkaiSetup, isToolKit} from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  'toolkit-name' = '@shinkai/toolkit-smtp';
  author = 'shinakai-dev';
  version = '0.0.1';

  // Register OAuth
  oauth = undefined;

  // Register Setup Keys
  executionSetup = [
    // Register Auth & Keys
    {
      name: 'HOST',
      type: DATA_TYPES.STRING,
      description: 'SMTP HOST e.g., smtp.gmail.com',
    },
    {
      name: 'PORT',
      type: DATA_TYPES.INTEGER,
      description: 'SMTP PORT. e.g., 587',
    },
    {
      name: 'EMAIL',
      type: DATA_TYPES.STRING,
      description: 'SMTP email address',
    },
    {
      name: 'PASSWORD',
      type: DATA_TYPES.STRING,
      description: 'SMTP password',
    },
    {
      name: 'TLS',
      type: DATA_TYPES.BOOLEAN,
      description: 'Use TLS',
    },
  ];
}
