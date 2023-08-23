import { ShinkaiSetup, isToolKit } from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  'toolkit-name' = '@shinkai/toolkit-web';
  author = 'shinkai-dev';
  version = '0.0.1';

  // Register OAuth
  oauth = undefined;

  // Register Setup Keys
  executionSetup = undefined;
}
