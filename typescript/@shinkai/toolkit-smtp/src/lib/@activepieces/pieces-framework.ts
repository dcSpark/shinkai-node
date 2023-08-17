import {OAuthShinkai} from '@shinkai/toolkit-lib';
import {
  SHINKAI_AP_HEADERS,
  SHINKAI_AP_INPUT,
} from './shinkai-activepieces-interface';

export const createAction = (setup: {
  auth: AuthTypeData | Record<string, any>;
  name: string;
  displayName: string;
  description: string;
  props: Record<string, any>;
  run: (context: Context) => Promise<any>;
}) => {
  // console.log('createAction', setup);
  return setup;
};

export type Context = {
  auth: SHINKAI_AP_HEADERS;
  propsValue: SHINKAI_AP_INPUT;
  webhookUrl?: string;
  store?: {
    put: <T>(x: string, y: T) => Promise<void>;
    get: <T>(x: string) => Promise<T>;
  };
};

export const createTrigger = (setup: {
  auth: AuthTypeData;
  name: string;
  displayName: string;
  description: string;
  props: Record<string, any>;
  sampleData: Record<string, any>;
  type: TriggerStrategy;
  onEnable: (context: Context) => Promise<any>;
  onDisable: (context: Context) => Promise<any>;
  run: (context: Context) => Promise<any>;
}) => {
  // console.log('createTrigger', setup);
  return setup;
};

export const createPiece = <T>(setup: T): T => {
  // console.log('createPiece', setup);
  return setup;
};

type AuthTypeData = OAuthShinkai;

export class PieceAuth {
  public static OAuth2(setup: AuthTypeData): AuthTypeData {
    // console.log('OAuth2', setup);
    return setup;
  }

  public static SecretText(setup: {displayName: string; required: boolean}) {
    // console.log('Checkbox', setup);
  }

  public static CustomAuth(setup: {
    displayName: string;
    required: boolean;
    props: Record<string, any>;
  }) {
    return setup;
  }
}

export enum TriggerStrategy {
  WEBHOOK = 'WEBHOOK',
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type OAuth2PropertyValue = any;

export class Property {
  public static Array(setup: {displayName: string; required: boolean}) {
    // console.log('Array', setup);
  }

  public static Checkbox(setup: {
    displayName: string;
    required: boolean;
    defaultValue: boolean;
  }) {
    // console.log('Checkbox', setup);
  }

  public static ShortText(setup: {
    displayName: string;
    required: boolean;
    description?: string;
  }) {
    // console.log('ShortText', setup);
  }
  public static DateTime(setup: {
    displayName: string;
    required: boolean;
    description?: string;
  }) {
    // console.log('DateTime', setup);
  }

  public static LongText(setup: {
    displayName: string;
    required: boolean;
    description?: string;
  }) {
    // console.log('LongText', setup);
  }

  public static Dropdown<T>(setup: {
    displayName: string;
    required: boolean;
    refreshers: any[];
    description?: string;
    options: (input: Record<string, any>) => Promise<{
      disabled: boolean;
      options: {
        label: string;
        value: T;
      }[];
    }>;
  }) {
    // console.log('Dropdown', setup);
  }

  public static StaticDropdown<T>(setup: {
    displayName: string;
    required: boolean;
    description?: string;
    options: {
      disabled: boolean;
      options: {
        label: string;
        value: T;
      }[];
    };
  }) {
    // console.log('StaticDropdown', setup);
  }
}
