import { DATA_TYPES, ShinkaiSetup, isToolKit } from '@shinkai/toolkit-lib';

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
  ];


  public async validateHeaders(
    headers: Record<string, string>
  ): Promise<boolean> {
    if (headers['api-key'] !== 'example') {
      throw new Error("Invalid api-key set");
    }
    return true;
  }
}
