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
  type?: DATA_TYPES;
}

export const SHINKAI_OAUTH = 'OAUTH';

export interface ShinkaiFieldIO extends ShinkaiField {
  context?: string;
  ebnf?: string;
}

export interface ShinkaiFieldHeader extends ShinkaiField {
  header?: string;
  oauth?: OAuthShinkai | undefined;
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
