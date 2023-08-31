import {ShinkaiSetup, isToolKit} from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  toolkitName = '@shinkai/toolkit-web';
  author = 'shinkai-dev';
  version = '0.0.1';

  // Define Headers
  toolkitHeaders = undefined;
}
