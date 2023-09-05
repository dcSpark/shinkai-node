import {ShinkaiToolkitLib} from './ShinkaiToolkitLib';
import {ShinkaiFieldHeader} from './types';

export abstract class ShinkaiSetup {
  abstract toolkitName: string;
  abstract author: string;
  abstract version: string;

  // List of fields that are required for the execution of the toolkit.
  // e.g., API Keys, OAuth, URLS, etc.
  toolkitHeaders?: ShinkaiFieldHeader[] | undefined;

  // Validate if header values are correct and valid.
  // e.g., API key must have a valid format and active.
  public async validateHeaders(
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    headers: Record<string, string>
  ): Promise<boolean> {
    await this.processRawHeaderValues(headers);
    return true;
  }

  public async processRawHeaderValues(rawHeader: Record<string, string>) {
    const v = await ShinkaiToolkitLib.getHeadersValidator();
    const headers = {};
    Object.keys(rawHeader).forEach((key: string) => {
      if (!v.transformer[key]) {
        // Skipping header. Not defined in in ShinkaiSetup.
      } else {
        Object.assign(headers, v.transformer[key](rawHeader[key]));
      }
    });
    const headerValidation = v.validator.validate(headers);
    if (headerValidation.error) {
      throw new Error(String(headerValidation.error));
    }
    return headerValidation.value;
  }
}
