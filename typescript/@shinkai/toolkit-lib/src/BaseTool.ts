import Joi from 'joi';
import {DecoratorsTools} from './Decortors';

export abstract class BaseInput {}
export abstract class BaseOutput {}
export abstract class BaseSetup {}

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
export abstract class BaseTool<
  I extends BaseInput,
  O extends BaseOutput,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  S extends BaseSetup = any
> {
  abstract description: string;
  oauth?: OAuthShinkai | undefined;

  abstract run(input: I, headers: S): Promise<O>;
  protected validate(input: I) {
    const validator: Joi.ObjectSchema = DecoratorsTools.getInputValidator(
      this.constructor.name
    );
    // console.log('validate', input, 'for', this.constructor.name, validator);
    const {value, error} = validator.validate(input);
    if (error) {
      throw new Error(String(error));
    }
  }
}
