import Joi from 'joi';
import {DecoratorsTools} from './Decortors';

export abstract class BaseInput {}
export abstract class BaseOutput {}

export abstract class BaseTool<I extends BaseInput, O extends BaseOutput> {
  abstract description: string;
  abstract run(input: I): Promise<O>;
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
