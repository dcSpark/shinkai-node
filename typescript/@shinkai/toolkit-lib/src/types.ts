export enum DATA_TYPES {
  BOOLEAN = 'BOOL',
  INTEGER = 'INT',
  FLOAT = 'FLOAT',
  STRING = 'STRING',
  ENUM = 'ENUM',
  CHAR = 'CHAR',
  JSON = 'JSON',
  ISODATE = 'ISODATE',
  // Special type for oauth headers
  OAUTH = 'OAUTH',
}

interface ShinkaiField {
  name: string;
  isOptional?: boolean;
  enum?: string[];
  description?: string;
  wrapperType?: 'none' | 'array';
}

export const SHINKAI_OAUTH = 'OAUTH';

export interface ShinkaiFieldIO extends ShinkaiField {
  context?: string;
  ebnf?: string;
  type?: DATA_TYPES;
}

export interface ShinkaiFieldHeader extends ShinkaiField {
  header?: string;
  oauth?: OAuthShinkai | undefined;
  type?: DATA_TYPES.STRING | DATA_TYPES.OAUTH;
}

export abstract class ShinkaiSetup {
  abstract 'toolkit-name': string;
  abstract author: string;
  abstract version: string;

  // List of fields that are required for the execution of the toolkit.
  // e.g., API Keys, OAuth, URLS, etc.
  executionSetup?: ShinkaiFieldHeader[] | undefined;

  // Validate if header values are correct and valid.
  // e.g., API key must have a valid format and active.
  public async validateHeaders(
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    headers: Record<string, string>
  ): Promise<boolean> {
    return true;
  }
}

export interface OAuthShinkai {
  authUrl: string;
  tokenUrl: string;
  required: boolean;
  pkce?: boolean | undefined;
  scope?: string[] | undefined;
  description?: string | undefined;
  cloudOAuth?: string | undefined;
  displayName?: string | undefined;
}
