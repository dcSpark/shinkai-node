import Joi from 'joi';
import {DecoratorsTools} from './DecortorsTools';

export abstract class BaseInput {}
export abstract class BaseOutput {}

export abstract class BaseTool<I extends BaseInput, O extends BaseOutput> {
  abstract description: string;

  abstract run(input: I, headers?: Record<string, string>): Promise<O>;
  protected async validate(input: I): Promise<void> {
    const validator: Joi.ObjectSchema = await DecoratorsTools.getInputValidator(
      this.constructor.name
    );
    const {error} = validator.validate(input);
    if (error) {
      throw new Error(String(error));
    }
  }
  protected async processHeaders(headers: Record<string, string>) {
    const validator = await DecoratorsTools.getHeadersValidator();
    const {error} = validator.validate(headers);
    if (error) {
      throw new Error(String(error));
    }
  }
}
