import {ShinkaiSetup, isToolKit} from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  toolkitName = 'Sample';
  author = 'dev';
  version = '0.0.1';

  // Register OAuth
  oauth = undefined;

  // Register Setup Keys
  toolkitHeaders = undefined;
}
