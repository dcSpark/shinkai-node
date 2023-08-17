import {
  isTool,
  isOutput,
  isInput,
  isBoolean,
  isInteger,
  BaseTool,
  BaseInput,
  BaseOutput,
} from '@shinkai/toolkit-lib';

@isInput('isEven')
class isEvenInput extends BaseInput {
  @isInteger('Integer number to check if is even.')
  number!: number;
}

@isOutput('isEven')
class isEvenOutput extends BaseOutput {
  @isBoolean('Result of the check. True if the number is even.')
  isEven!: boolean;
}

@isTool
export class isEven extends BaseTool<isEvenInput, isEvenOutput> {
  description = 'Check if a number is even';

  async run(input: isEvenInput): Promise<isEvenOutput> {
    await this.validate(input);

    const isEven = (input.number || 0) % 2 === 0;
    return {isEven} as any;
  }
}
