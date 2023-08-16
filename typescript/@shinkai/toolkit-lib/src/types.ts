export enum DATA_TYPES {
  BOOLEAN = 'BOOL',
  INTEGER = 'INT',
  FLOAT = 'FLOAT',
  STRING = 'STRING',
  ENUM = 'ENUM',
  CHAR = 'CHAR',
  JSON = 'JSON',
  ISODATE = 'ISODATE',
}

export interface ShinkaiField {
  name: string;
  context?: string;
  type?: DATA_TYPES;
  isOptional?: boolean;
  enum?: string[];
  description?: string;
  wrapperType?: 'none' | 'array';
  ebnf?: string;
  header?: string;
}

export abstract class ShinkaiSetup {
  abstract 'toolkit-name': string;
  abstract author: string;
  abstract version: string;
  abstract oauth?: OAuthShinkai | undefined;
  abstract executionSetup?: ShinkaiField[] | undefined;
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
