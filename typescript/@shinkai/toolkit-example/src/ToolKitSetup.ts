import {DATA_TYPES, ShinkaiSetup, isToolKit} from '@shinkai/toolkit-lib';

@isToolKit
export class ToolKitSetup extends ShinkaiSetup {
  toolkitName = 'toolkit-example';
  author = 'shinkai-dev';
  version = '0.0.1';

  // Define Headers
  toolkitHeaders = [
    {
      name: 'api-key',
      type: DATA_TYPES.STRING,
      description: 'An example api-key header',
    },
    {
      name: 'example-bool',
      description: 'Example boolean header',
      type: DATA_TYPES.BOOLEAN,
    },
  ];

  public async validateHeaders(
    headers: Record<string, string>
  ): Promise<boolean> {
    if (headers['x-shinkai-api-key'] !== 'example') {
      throw new Error("Invalid 'api-key' header");
    }
    if (String(headers['example-bool']) === String(false)) {
      throw new Error("Invalid 'example-bool' header");
    }
    if (String(headers['example-bool']) === String(true)) {
      return Promise.resolve(true);
    }
    return Promise.resolve(true);
  }
}
