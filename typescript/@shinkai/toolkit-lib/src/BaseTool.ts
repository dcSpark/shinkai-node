import Joi from 'joi';
import {ShinkaiToolkitLib} from './ShinkaiToolkitLib';

export abstract class BaseInput {}
export abstract class BaseOutput {
  public async processOutput(): Promise<{}[]> {
    const config = await ShinkaiToolkitLib.emitConfig();
    const toolName = ShinkaiToolkitLib.findToolByOutput(this.constructor.name);
    const tool = config.tools.find(t => t.name === toolName);
    if (!tool) {
      throw new Error(`Tool ${toolName} not found`);
    }

    return tool.output.map(o => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (o as any).result = (this as any)[o.name];
      return o;
    });
  }
}

export abstract class BaseTool<I extends BaseInput, O extends BaseOutput> {
  abstract description: string;

  abstract run(input: I, headers: Record<string, string>): Promise<O>;

  public async validateInputs(input: I): Promise<I> {
    const validator: Joi.ObjectSchema =
      await ShinkaiToolkitLib.getInputValidator(this.constructor.name);
    const inputValidation = validator.validate(input);
    if (inputValidation.error) {
      throw new Error(String(inputValidation.error));
    }
    return inputValidation.value;
  }
}
