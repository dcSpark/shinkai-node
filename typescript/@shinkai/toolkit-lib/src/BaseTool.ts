import Joi from 'joi';
import {ShinkaiTookitLib} from './ShinkaiTookitLib';

export abstract class BaseInput {}
export abstract class BaseOutput {}

export abstract class BaseTool<I extends BaseInput, O extends BaseOutput> {
  abstract description: string;

  abstract run(input: I, headers: Record<string, string>): Promise<O>;

  public async validateInputs(input: I): Promise<I> {
    const validator: Joi.ObjectSchema =
      await ShinkaiTookitLib.getInputValidator(this.constructor.name);
    const inputValidation = validator.validate(input);
    if (inputValidation.error) {
      throw new Error(String(inputValidation.error));
    }
    return inputValidation.value;
  }
}
