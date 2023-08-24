import { ShinkaiSetup, isToolKit } from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  'toolkit-name' = 'Sample';
  author = 'dev';
  version = '0.0.1';

  // Register OAuth
  oauth = undefined;

  // Register Setup Keys
  toolkitHeaders = undefined;
}
